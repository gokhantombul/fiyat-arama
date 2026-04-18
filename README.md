# fiyat-arama

Rust ile geliştirilmiş, finansal veri / ürün arama odaklı interaktif CLI + REPL + TUI prototipi.

## Özellikler
- Argümansız çalıştırınca etkileşimli ana menü (ok tuşları ile seçim)
- Komut satırından doğrudan subcommand çalıştırma
- REPL modu (`fiyat-arama>`) ile art arda komut girme
- `incele <URL>` ile genel metadata + JSON-LD inceleme

## Gereksinimler
- Rust (önerilen: stable)
- Cargo

## Kurulum
```bash
# Rust yoksa
brew install rustup-init
rustup-init -y
source "$HOME/.cargo/env"

# Projeyi çalıştır
cargo build
```

## Çalıştırma

### 1) İnteraktif menü (önerilen)
```bash
cargo run
```

Menü seçenekleri:
1. Döviz Kurları
2. Benzin Fiyatları
3. Ürün Ara
4. Site İncele
5. REPL Komut Modu
6. Çıkış

### 2) Doğrudan komut ile
```bash
cargo run -- doviz
cargo run -- benzin istanbul kadikoy --sirala
cargo run -- ara "iphone 15"
cargo run -- incele "https://example.com/product"
```

### 3) REPL modu
`cargo run` ile uygulamayı başlatıp menüden **REPL Komut Modu** seçin.

REPL örnekleri:
```text
fiyat-arama> help
fiyat-arama> doviz
fiyat-arama> benzin izmir karsiyaka --sirala
fiyat-arama> ara "filtre kahve"
fiyat-arama> incele https://example.com
fiyat-arama> exit
```

## Komutlar

### `doviz`
Döviz kurları için tablo çıktısı üretir (şu an demo veri).

### `benzin [sehir] [ilce] --sirala`
Benzin fiyatlarını şehir/ilçe filtresiyle listeler (şu an demo çıktı).

### `ara [urun_adi]`
Ürün arama akışını çalıştırır (şu an demo çıktı, gerçek entegrasyon için hazır iskelet).

### `incele <URL>`
Verilen URL için:
- `<title>`
- meta description
- OpenGraph etiketleri
- `application/ld+json` script içerikleri

alanlarını çıkarır. JSON-LD içinde `Product` veya `Article` tespit ederse tablo halinde gösterir.

## Notlar
- Bu sürümde `doviz`, `benzin` ve `ara` komutları entegrasyon iskeleti olarak bırakılmıştır.
- Ağ/proxy kısıtı olan ortamlarda `cargo build/check` bağımlılık indirmede hata verebilir.
