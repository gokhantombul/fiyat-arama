use std::{collections::BTreeMap, error::Error};

use clap::{Parser, Subcommand};
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use inquire::Select;
use reqwest::Client;
use rustyline::{error::ReadlineError, DefaultEditor};
use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "fiyat-arama")]
#[command(about = "Finansal veri takipçisi, ürün arama ve site inceleme aracı", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
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

async fn run_interactive_menu() -> Result<(), Box<dyn Error>> {
    loop {
        let menu_items = vec![
            "1. Döviz Kurları",
            "2. Benzin Fiyatları",
            "3. Ürün Ara",
            "4. Site İncele",
            "5. REPL Komut Modu",
            "6. Çıkış",
        ];

        let selection = Select::new("Ne yapmak istersiniz?", menu_items.clone()).prompt();

        match selection {
            Ok("1. Döviz Kurları") => execute_command(Commands::Doviz).await?,
            Ok("2. Benzin Fiyatları") => {
                let sehir = inquire::Text::new("Şehir (opsiyonel):").prompt().ok();
                let ilce = inquire::Text::new("İlçe (opsiyonel):").prompt().ok();
                execute_command(Commands::Benzin {
                    sehir: sehir.filter(|s| !s.trim().is_empty()),
                    ilce: ilce.filter(|s| !s.trim().is_empty()),
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

async fn run_repl() -> Result<(), Box<dyn Error>> {
    let mut rl = DefaultEditor::new()?;
    println!("REPL moduna hoş geldiniz. Yardım için 'help' yazın, çıkmak için 'exit' yazın.");

    loop {
        let line = rl.readline("fiyat-arama> ");

        match line {
            Ok(input) => {
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input)?;

                if matches!(input, "exit" | "quit") {
                    break;
                }
                if input.eq_ignore_ascii_case("help") {
                    print_repl_help();
                    continue;
                }

                let args = match shlex::split(input) {
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

fn print_repl_help() {
    println!("\nKullanılabilir komutlar:");
    println!("  doviz");
    println!("  benzin [sehir] [ilce] [--sirala]");
    println!("  ara [urun_adi]");
    println!("  incele <URL>");
    println!("  help");
    println!("  exit | quit\n");
}

async fn execute_command(command: Commands) -> Result<(), Box<dyn Error>> {
    match command {
        Commands::Doviz => {
            println!("Döviz scraping demo çıktısı:");
            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec!["Birim", "Alış", "Satış"]);

            table.add_row(vec![
                Cell::new("USD/TRY").fg(Color::Green),
                Cell::new("38.42"),
                Cell::new("38.47"),
            ]);
            table.add_row(vec![
                Cell::new("EUR/TRY").fg(Color::Cyan),
                Cell::new("43.60"),
                Cell::new("43.68"),
            ]);
            println!("{table}");
        }
        Commands::Benzin {
            sehir,
            ilce,
            sirala,
        } => {
            println!(
                "Benzin scraping demo -> şehir: {:?}, ilçe: {:?}, sırala: {}",
                sehir, ilce, sirala
            );
        }
        Commands::Ara { urun_adi } => {
            let query = urun_adi.join(" ");
            if query.trim().is_empty() {
                eprintln!("Kullanım: ara [urun_adi]");
                return Ok(());
            }
            println!("Genel arama demo -> {query}");
            println!("(Bu bölümde gerçek entegrasyonda fiyatlar ucuzdan pahalıya sıralanmalıdır.)");
        }
        Commands::Incele { url } => {
            let inspection = inspect_url(&url).await?;
            render_inspection(&inspection);
        }
    }

    Ok(())
}

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
    println!("\n=== Site İnceleme Raporu ===");
    println!("URL: {}", inspection.url);
    println!("Başlık: {}", inspection.title.as_deref().unwrap_or("-"));
    println!(
        "Açıklama: {}",
        inspection.description.as_deref().unwrap_or("-")
    );

    if !inspection.open_graph.is_empty() {
        println!("\nOpenGraph Etiketleri:");
        let mut og_table = Table::new();
        og_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec!["Etiket", "Değer"]);

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
        println!("\nTespit: Product");
        let mut p_table = Table::new();
        p_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec!["Alan", "Değer"]);

        p_table.add_row(vec![
            Cell::new("Name").fg(Color::Green),
            Cell::new(&product.name),
        ]);
        p_table.add_row(vec![
            Cell::new("Price"),
            Cell::new(product.price.as_deref().unwrap_or("-")),
        ]);
        p_table.add_row(vec![
            Cell::new("Currency"),
            Cell::new(product.currency.as_deref().unwrap_or("-")),
        ]);
        p_table.add_row(vec![
            Cell::new("Availability"),
            Cell::new(product.availability.as_deref().unwrap_or("-")),
        ]);
        println!("{p_table}");
    } else if let Some(article) = &inspection.detected_article {
        println!("\nTespit: Article");
        let mut a_table = Table::new();
        a_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec!["Alan", "Değer"]);

        a_table.add_row(vec![
            Cell::new("Headline").fg(Color::Yellow),
            Cell::new(article.headline.as_deref().unwrap_or("-")),
        ]);
        a_table.add_row(vec![
            Cell::new("Author"),
            Cell::new(article.author.as_deref().unwrap_or("-")),
        ]);
        a_table.add_row(vec![
            Cell::new("DatePublished"),
            Cell::new(article.date_published.as_deref().unwrap_or("-")),
        ]);
        println!("{a_table}");
    } else {
        println!("\nJSON-LD içinde Product veya Article tipi bulunamadı.");
    }
}
