# xml_schema_finder

Reads through an XML file, and outputs information to help you understand the structure

E.g, scanning the full 50GB wikipedia dump takes ~3-4minutes, but in practice under a second will give you a strong starting point.
Certainly beats trying to find documentation on the file format or wading through a huge xml file for the structure!

Use something like:
```
cargo run --release -- -f /Users/b.shaw/Downloads/enwiki-20170820-pages-articles.xml -n 10000000
```
