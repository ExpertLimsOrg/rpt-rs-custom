# The `rpt-codes` CLI

The `rpt-codes-cli` app (`apps/rpt-codes-cli`) builds the `rpt-codes` binary: a small utility that
writes a **PNG** image of either a **QR code** or a **Code 39** barcode. It does not open a Crystal
`.rpt` file and does not call the render pipeline.

```
rpt-codes [OPTIONS]

MODES (exactly one)
    -qr, --qr <BASE64>    QR code; BASE64 uses the standard alphabet (with `=` padding when needed)
                           and decodes to the target URL as UTF-8 (max 4096 bytes decoded)
    -code, --code          Code 39 barcode; requires -value / --value

PAYLOAD
    -value, --value <TEXT>   Code 39 data: `0-9`, `A-Z`, space, and `- . $ / + %`

SIZE
    --width-mm <N>    output width in millimetres (required)
    --height-mm <N>   output height in millimetres (required); for QR must equal --width-mm
    --dpi <N>         resolution used to convert mm to pixels (default: 300)

OUTPUT
    -o, --output <path>   destination file; `-` or omit → stdout (binary PNG)

    -h, --help        show help
    -V, --version     show version and exit
```

## Examples

```sh
# QR for a URL (URL passed in base64)
rpt-codes -o qr.png --width-mm 40 --height-mm 40 \
  --qr "$(printf '%s' 'https://example.com' | base64)"

# Code 39
rpt-codes -o sample.png --width-mm 80 --height-mm 25 --code --value 'SAMPLE-001'
```

Invalid flags or missing required options exit with code **2** (same convention as `rpt-render`).
Rendering failures exit with code **1**.