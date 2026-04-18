# fiyat-arama

`fiyat-arama`, terminalden finansal veri takibi, ürün arama ve URL metadata inceleme için hazırlanmış bir Rust CLI uygulamasıdır.

## Kurulum

```bash
cargo build --release
```

## Kullanım

```bash
cargo run -- help
cargo run -- doviz
cargo run -- benzin istanbul kadikoy --sirala
cargo run -- ara "iphone 15"
cargo run -- incele https://example.com
```

Ayrıca etkileşimli menü için:

```bash
cargo run
```

## Komutlar

- `help`: Uygulama komut kullanım özetini gösterir.
- `doviz`: Döviz kuru demo çıktısını gösterir.
- `benzin [sehir] [ilce] [--sirala]`: Benzin fiyatı demo çıktısını gösterir.
- `ara <urun_adi...>`: Ürün arama demo çıktısını verir.
- `incele <url>`: Verilen URL için metadata ve JSON-LD analizi yapar.

## Notlar

- Standart Clap yardımı için `--help` kullanılabilir.
- `incele` komutu gerçek HTTP isteği attığı için internet bağlantısı gerektirir.
