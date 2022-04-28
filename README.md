Small Rust utility to convert .TAP format into .TZX format.

This assumes that a TAP file is small enough to fit into main memory.
If that's not universally true, then please raise an issue and I'll make it used buffered IO instead.

# Building
```
cargo build --release
```

# Usage
```
./tap2tzx input.tap [output.tzx]

input.tap  - mandatory path to input TAP file
output.tzx - optional path to output tzx location. If omitted, an
             output file path based on a tzx-equivalent of the input
             file will created/overwritten
```

# References

* [TAP format](https://sinclair.wiki.zxnet.co.uk/wiki/TAP_format)
* [TZX Format](https://worldofspectrum.net/TZXformat.html)
