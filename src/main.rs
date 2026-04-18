use std::{collections::BTreeMap, error::Error};

use clap::{Parser, Subcommand};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Select;
use reqwest::Client;
use rustyline::{
    completion::{Completer, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::Hinter,
    history::DefaultHistory,
    validate::Validator,
    Context, Editor, Helper,
};
use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "fiyat-arama")]
#[command(about = "Finansal veri takipçisi, ürün arama ve site inceleme aracı", long_about = None)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Komut kullanım yardımını göster
    Help,
    /// Döviz kurlarını getir
    Doviz,
    /// Benzin fiyatlarını getir
    Benzin {
        sehir: Option<String>,
        ilce: Option<String>,
        #[arg(long)]
        sirala: bool,
    },
    /// Ürün ara (en ucuzdan pahalıya)
    Ara { urun_adi: Vec<String> },
    /// Rastgele bir URL'in metadata ve JSON-LD bilgisini incele
    Incele { url: String },
}

#[derive(Debug, Clone)]
struct SehirBilgi {
    ad: String,
    slug: String,
}

#[derive(Debug, Clone)]
struct BenzinFiyati {
    dagitici: String,
    benzin: String,
    motorin: String,
    lpg: String,
}

#[derive(Debug, Clone)]
struct MetaInspection {
    url: String,
    title: Option<String>,
    description: Option<String>,
    open_graph: BTreeMap<String, String>,
    json_ld_objects: Vec<Value>,
    detected_product: Option<ProductInfo>,
    detected_article: Option<ArticleInfo>,
}

#[derive(Debug, Clone)]
struct ProductInfo {
    name: String,
    price: Option<String>,
    currency: Option<String>,
    availability: Option<String>,
}

#[derive(Debug, Clone)]
struct ArticleInfo {
    headline: Option<String>,
    author: Option<String>,
    date_published: Option<String>,
}

// ── REPL Tab Tamamlama ───────────────────────────────────────────────────────

struct FiyatCompleter {
    commands: Vec<String>,
}

impl Completer for FiyatCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let word_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let word = &line[word_start..pos];

        let matches: Vec<Pair> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(word))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect();

        Ok((word_start, matches))
    }
}

impl Hinter for FiyatCompleter {
    type Hint = String;
}
impl Highlighter for FiyatCompleter {}
impl Validator for FiyatCompleter {}
impl Helper for FiyatCompleter {}

// ── Yardımcılar ─────────────────────────────────────────────────────────────

fn yeni_spinner(mesaj: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(mesaj.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

fn bolum_basligi(baslik: &str) {
    let cizgi = "─".repeat(baslik.len() + 4);
    println!("\n┌{cizgi}┐");
    println!("│  {baslik}  │");
    println!("└{cizgi}┘");
}

// ── Ana Giriş ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        if let Err(err) = execute_command(command).await {
            eprintln!("Hata: {err}");
        }
        return;
    }

    if let Err(err) = run_interactive_menu().await {
        eprintln!("Hata: {err}");
    }
}

// ── Menü ────────────────────────────────────────────────────────────────────

async fn run_interactive_menu() -> Result<(), Box<dyn Error>> {
    loop {
        let menu_items = vec![
            "0. Yardım",
            "1. Döviz Kurları",
            "2. Benzin Fiyatları",
            "3. Ürün Ara",
            "4. Site İncele",
            "5. REPL Komut Modu",
            "6. Çıkış",
        ];

        let selection = Select::new("Ne yapmak istersiniz?", menu_items.clone()).prompt();

        match selection {
            Ok("0. Yardım") => execute_command(Commands::Help).await?,
            Ok("1. Döviz Kurları") => execute_command(Commands::Doviz).await?,
            Ok("2. Benzin Fiyatları") => {
                execute_command(Commands::Benzin {
                    sehir: None,
                    ilce: None,
                    sirala: true,
                })
                .await?;
            }
            Ok("3. Ürün Ara") => {
                let urun = inquire::Text::new("Ürün adı:").prompt()?;
                execute_command(Commands::Ara {
                    urun_adi: vec![urun],
                })
                .await?;
            }
            Ok("4. Site İncele") => {
                let url = inquire::Text::new("URL:").prompt()?;
                execute_command(Commands::Incele { url }).await?;
            }
            Ok("5. REPL Komut Modu") => run_repl().await?,
            Ok("6. Çıkış") => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}

// ── REPL ────────────────────────────────────────────────────────────────────

async fn run_repl() -> Result<(), Box<dyn Error>> {
    let helper = FiyatCompleter {
        commands: vec![
            "help".to_string(),
            "doviz".to_string(),
            "benzin".to_string(),
            "ara".to_string(),
            "incele".to_string(),
            "exit".to_string(),
            "quit".to_string(),
        ],
    };

    let mut rl: Editor<FiyatCompleter, DefaultHistory> = Editor::new()?;
    rl.set_helper(Some(helper));

    println!("REPL moduna hoş geldiniz. Çıkmak için 'exit' yazın.");
    println!("  Tab  → komut tamamlama   |   ↑↓  → geçmiş");

    loop {
        let line = rl.readline("fiyat-arama> ");

        match line {
            Ok(input) => {
                let input = input.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(&input)?;

                if matches!(input.as_str(), "exit" | "quit") {
                    break;
                }

                let args = match shlex::split(&input) {
                    Some(a) => a,
                    None => {
                        eprintln!("Komut ayrıştırılamadı.");
                        continue;
                    }
                };

                let mut argv = vec!["fiyat-arama".to_string()];
                argv.extend(args);

                match Cli::try_parse_from(argv) {
                    Ok(parsed) => {
                        if let Some(cmd) = parsed.command {
                            if let Err(err) = execute_command(cmd).await {
                                eprintln!("Komut hatası: {err}");
                            }
                        }
                    }
                    Err(err) => eprintln!("{err}"),
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(err) => {
                eprintln!("REPL hatası: {err}");
                break;
            }
        }
    }

    Ok(())
}

// ── Komut İşleyici ──────────────────────────────────────────────────────────

async fn execute_command(command: Commands) -> Result<(), Box<dyn Error>> {
    match command {
        Commands::Help => print_help_message(),

        Commands::Doviz => {
            bolum_basligi("Döviz Kurları (demo)");
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec![
                    Cell::new("Birim").fg(Color::Cyan),
                    Cell::new("Alış").fg(Color::Cyan),
                    Cell::new("Satış").fg(Color::Cyan),
                ]);
            table.add_row(vec![
                Cell::new("USD/TRY").fg(Color::Green),
                Cell::new("38.42"),
                Cell::new("38.47"),
            ]);
            table.add_row(vec![
                Cell::new("EUR/TRY").fg(Color::Blue),
                Cell::new("43.60"),
                Cell::new("43.68"),
            ]);
            table.add_row(vec![
                Cell::new("GBP/TRY").fg(Color::Magenta),
                Cell::new("50.10"),
                Cell::new("50.20"),
            ]);
            println!("{table}");
        }

        Commands::Benzin {
            sehir,
            ilce: _,
            sirala: _,
        } => {
            let client = Client::builder().user_agent("fiyat-arama/0.1").build()?;

            let sehir_slug = if let Some(s) = sehir {
                s
            } else {
                benzin_konum_sec(&client).await?
            };

            let pb = yeni_spinner(&format!("Fiyatlar alınıyor: {}...", sehir_slug));
            let fiyatlar = fetch_benzin_fiyatlari(&client, &sehir_slug).await?;
            pb.finish_and_clear();

            if fiyatlar.is_empty() {
                println!("Bu konum için fiyat verisi bulunamadı.");
            } else {
                render_benzin_tablosu(&fiyatlar, &sehir_slug);
            }
        }

        Commands::Ara { urun_adi } => {
            let query = urun_adi.join(" ");
            if query.trim().is_empty() {
                eprintln!("Kullanım: ara [urun_adi]");
                return Ok(());
            }
            bolum_basligi(&format!("Ürün Arama: {query}"));
            println!("(Gerçek entegrasyonda fiyatlar ucuzdan pahalıya sıralanacak.)");
        }

        Commands::Incele { url } => {
            let pb = yeni_spinner("Site inceleniyor...");
            let inspection = inspect_url(&url).await?;
            pb.finish_and_clear();
            render_inspection(&inspection);
        }
    }

    Ok(())
}

// ── Benzin: Konum Seçimi ────────────────────────────────────────────────────

async fn benzin_konum_sec(client: &Client) -> Result<String, Box<dyn Error>> {
    let pb = yeni_spinner("Şehirler doviz.com'dan yükleniyor...");
    let sehirler = fetch_benzin_sehirler(client).await?;
    pb.finish_and_clear();

    if sehirler.is_empty() {
        return Err("Şehir listesi alınamadı.".into());
    }

    let sehir_adlari: Vec<String> = sehirler.iter().map(|s| s.ad.clone()).collect();
    let secilen_ad = Select::new("Şehir seçin:", sehir_adlari)
        .with_help_message("Filtrelemek için yazmaya başlayın, ↑↓ ile seçin, Enter ile onayla")
        .prompt()?;

    let sehir_slug = sehirler
        .iter()
        .find(|s| s.ad == secilen_ad)
        .map(|s| s.slug.clone())
        .unwrap_or_default();

    Ok(sehir_slug)
}

// ── Benzin: Scraping ────────────────────────────────────────────────────────

async fn fetch_benzin_sehirler(client: &Client) -> Result<Vec<SehirBilgi>, Box<dyn Error>> {
    let html = client
        .get("https://www.doviz.com/akaryakit-fiyatlari")
        .send()
        .await?
        .text()
        .await?;

    let doc = Html::parse_document(&html);
    let link_sel = Selector::parse("a[href]").unwrap();

    let mut sehirler: Vec<SehirBilgi> = Vec::new();
    let mut gorulenler = std::collections::HashSet::new();
    let prefix = "https://www.doviz.com/akaryakit-fiyatlari/";

    for el in doc.select(&link_sel) {
        let href = el.value().attr("href").unwrap_or("");
        if !href.starts_with(prefix) {
            continue;
        }
        let slug = href[prefix.len()..].trim_matches('/').to_string();
        if slug.is_empty() || slug.contains('/') || !gorulenler.insert(slug.clone()) {
            continue;
        }
        let ad = el.text().collect::<String>().trim().to_string();
        if !ad.is_empty() && ad.len() < 60 {
            sehirler.push(SehirBilgi { ad, slug });
        }
    }

    Ok(sehirler)
}


async fn fetch_benzin_fiyatlari(
    client: &Client,
    sehir_slug: &str,
) -> Result<Vec<BenzinFiyati>, Box<dyn Error>> {
    let url = format!("https://www.doviz.com/akaryakit-fiyatlari/{}", sehir_slug);
    let html = client.get(&url).send().await?.text().await?;
    let doc = Html::parse_document(&html);

    let mut fiyatlar: Vec<BenzinFiyati> = Vec::new();
    let row_sel = Selector::parse("table tbody tr").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    for row in doc.select(&row_sel) {
        let cells: Vec<String> = row
            .select(&td_sel)
            .map(|td| td.text().collect::<String>().trim().to_string())
            .collect();

        if cells.len() >= 4 {
            fiyatlar.push(BenzinFiyati {
                dagitici: cells[0].clone(),
                benzin: cells[1].clone(),
                motorin: cells[2].clone(),
                lpg: cells[3].clone(),
            });
        }
    }

    Ok(fiyatlar)
}

fn render_benzin_tablosu(fiyatlar: &[BenzinFiyati], konum: &str) {
    bolum_basligi(&format!("Benzin Fiyatları — {konum}"));

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Dağıtıcı").fg(Color::Cyan),
            Cell::new("Benzin (₺)").fg(Color::Cyan),
            Cell::new("Motorin (₺)").fg(Color::Cyan),
            Cell::new("LPG (₺)").fg(Color::Cyan),
        ]);

    for fiyat in fiyatlar {
        table.add_row(vec![
            Cell::new(&fiyat.dagitici),
            Cell::new(&fiyat.benzin).fg(Color::Green),
            Cell::new(&fiyat.motorin).fg(Color::Yellow),
            Cell::new(&fiyat.lpg).fg(Color::Magenta),
        ]);
    }

    println!("{table}");
}

// ── Yardım ──────────────────────────────────────────────────────────────────

fn print_help_message() {
    bolum_basligi("fiyat-arama Komutları");
    println!("  fiyat-arama help");
    println!("  fiyat-arama doviz");
    println!("  fiyat-arama benzin [SEHIR] [ILCE] [--sirala]");
    println!("  fiyat-arama ara <URUN_ADI...>");
    println!("  fiyat-arama incele <URL>");
    println!();
    println!("İpucu: Standart yardım için `fiyat-arama --help` de çalışır.");
}

// ── Site İnceleme ────────────────────────────────────────────────────────────

async fn inspect_url(raw_url: &str) -> Result<MetaInspection, Box<dyn Error>> {
    let validated_url = Url::parse(raw_url)?;
    let client = Client::builder().user_agent("fiyat-arama/0.1").build()?;
    let body = client
        .get(validated_url.clone())
        .send()
        .await?
        .text()
        .await?;

    let document = Html::parse_document(&body);
    let title_selector = Selector::parse("title").unwrap();
    let meta_selector = Selector::parse("meta").unwrap();
    let json_ld_selector = Selector::parse("script[type='application/ld+json']").unwrap();

    let title = document
        .select(&title_selector)
        .next()
        .map(|n| n.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    let mut description = None;
    let mut open_graph = BTreeMap::new();

    for meta in document.select(&meta_selector) {
        let value = meta.value();
        let content = value.attr("content").unwrap_or("").trim().to_string();
        if content.is_empty() {
            continue;
        }

        if let Some(name) = value.attr("name") {
            if name.eq_ignore_ascii_case("description") {
                description = Some(content.clone());
            }
        }

        if let Some(property) = value.attr("property") {
            if property.starts_with("og:") {
                open_graph.insert(property.to_string(), content);
            }
        }
    }

    let mut json_ld_objects = Vec::new();
    for node in document.select(&json_ld_selector) {
        let json_text = node.inner_html();
        if let Ok(json) = serde_json::from_str::<Value>(&json_text) {
            match json {
                Value::Array(items) => json_ld_objects.extend(items),
                other => json_ld_objects.push(other),
            }
        }
    }

    let detected_product = detect_product_from_jsonld(&json_ld_objects);
    let detected_article = detect_article_from_jsonld(&json_ld_objects);

    Ok(MetaInspection {
        url: validated_url.to_string(),
        title,
        description,
        open_graph,
        json_ld_objects,
        detected_product,
        detected_article,
    })
}

fn detect_product_from_jsonld(items: &[Value]) -> Option<ProductInfo> {
    for item in items {
        if !json_ld_has_type(item, "Product") {
            continue;
        }

        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(adı bulunamadı)")
            .to_string();

        let offers = item.get("offers");
        let (price, currency, availability) = extract_offer_fields(offers);

        return Some(ProductInfo {
            name,
            price,
            currency,
            availability,
        });
    }

    None
}

fn detect_article_from_jsonld(items: &[Value]) -> Option<ArticleInfo> {
    for item in items {
        if !json_ld_has_type(item, "Article") {
            continue;
        }

        let headline = item
            .get("headline")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let author = match item.get("author") {
            Some(Value::String(s)) => Some(s.to_string()),
            Some(Value::Object(map)) => map
                .get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            Some(Value::Array(arr)) => arr.iter().find_map(|v| {
                v.get("name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            }),
            _ => None,
        };

        let date_published = item
            .get("datePublished")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        return Some(ArticleInfo {
            headline,
            author,
            date_published,
        });
    }

    None
}

fn json_ld_has_type(item: &Value, expected: &str) -> bool {
    match item.get("@type") {
        Some(Value::String(t)) => t.eq_ignore_ascii_case(expected),
        Some(Value::Array(arr)) => arr
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case(expected))),
        _ => false,
    }
}

fn extract_offer_fields(
    offers: Option<&Value>,
) -> (Option<String>, Option<String>, Option<String>) {
    let offer = match offers {
        Some(Value::Object(_)) => offers,
        Some(Value::Array(arr)) => arr.first(),
        _ => None,
    };

    let price = offer.and_then(|o| o.get("price")).and_then(|v| match v {
        Value::String(s) => Some(s.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    });

    let currency = offer
        .and_then(|o| o.get("priceCurrency"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let availability = offer
        .and_then(|o| o.get("availability"))
        .and_then(Value::as_str)
        .map(shorten_schema_url);

    (price, currency, availability)
}

fn shorten_schema_url(value: &str) -> String {
    value.rsplit('/').next().unwrap_or(value).to_string()
}

fn render_inspection(inspection: &MetaInspection) {
    bolum_basligi("Site İnceleme Raporu");

    let mut info_table = Table::new();
    info_table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Alan").fg(Color::Cyan),
            Cell::new("Değer").fg(Color::Cyan),
        ]);
    info_table.add_row(vec![
        Cell::new("URL").fg(Color::Blue),
        Cell::new(&inspection.url),
    ]);
    info_table.add_row(vec![
        Cell::new("Başlık").fg(Color::Blue),
        Cell::new(inspection.title.as_deref().unwrap_or("—")),
    ]);
    info_table.add_row(vec![
        Cell::new("Açıklama").fg(Color::Blue),
        Cell::new(inspection.description.as_deref().unwrap_or("—")),
    ]);
    println!("{info_table}");

    if !inspection.open_graph.is_empty() {
        println!("\nOpenGraph Etiketleri:");
        let mut og_table = Table::new();
        og_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("Etiket").fg(Color::Cyan),
                Cell::new("Değer").fg(Color::Cyan),
            ]);

        for (k, v) in &inspection.open_graph {
            og_table.add_row(vec![Cell::new(k).fg(Color::Blue), Cell::new(v)]);
        }
        println!("{og_table}");
    }

    println!(
        "\nJSON-LD Script Adedi: {}",
        inspection.json_ld_objects.len()
    );

    if let Some(product) = &inspection.detected_product {
        println!("\nTespit: Ürün (Product)");
        let mut p_table = Table::new();
        p_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("Alan").fg(Color::Cyan),
                Cell::new("Değer").fg(Color::Cyan),
            ]);
        p_table.add_row(vec![
            Cell::new("İsim").fg(Color::Green),
            Cell::new(&product.name),
        ]);
        p_table.add_row(vec![
            Cell::new("Fiyat"),
            Cell::new(product.price.as_deref().unwrap_or("—")),
        ]);
        p_table.add_row(vec![
            Cell::new("Para Birimi"),
            Cell::new(product.currency.as_deref().unwrap_or("—")),
        ]);
        p_table.add_row(vec![
            Cell::new("Stok Durumu"),
            Cell::new(product.availability.as_deref().unwrap_or("—")),
        ]);
        println!("{p_table}");
    } else if let Some(article) = &inspection.detected_article {
        println!("\nTespit: Makale (Article)");
        let mut a_table = Table::new();
        a_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("Alan").fg(Color::Cyan),
                Cell::new("Değer").fg(Color::Cyan),
            ]);
        a_table.add_row(vec![
            Cell::new("Başlık").fg(Color::Yellow),
            Cell::new(article.headline.as_deref().unwrap_or("—")),
        ]);
        a_table.add_row(vec![
            Cell::new("Yazar"),
            Cell::new(article.author.as_deref().unwrap_or("—")),
        ]);
        a_table.add_row(vec![
            Cell::new("Yayın Tarihi"),
            Cell::new(article.date_published.as_deref().unwrap_or("—")),
        ]);
        println!("{a_table}");
    } else {
        println!("\nJSON-LD içinde Product veya Article tipi bulunamadı.");
    }
}
