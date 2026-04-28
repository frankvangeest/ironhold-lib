# AVIF to PNG Converter

Batch-converts `.avif` files to `.png`. Requires Pillow 11+ (built-in AVIF support — no extra plugin needed).

## Install

```bash
pip install -r tools/avif2png/requirements.txt
```

## Usage

```bash
# Convert all AVIFs in a directory (output next to each source)
python tools/avif2png/convert.py assets/shared/models/avif/

# Convert to a different output directory
python tools/avif2png/convert.py assets/shared/models/avif/ --output-dir assets/shared/models/

# Convert a single file
python tools/avif2png/convert.py path/to/image.avif

# Force overwrite of existing PNGs
python tools/avif2png/convert.py assets/shared/models/avif/ --force

# Resize to thumbnail (longest side ≤ 256 px)
python tools/avif2png/convert.py assets/shared/models/avif/ --max-size 256
```

## Behaviour

- Skips files that already have a matching PNG (use `--force` to overwrite).
- Preserves alpha (RGBA → PNG with transparency).
- `--max-size` resizes using high-quality Lanczos downsampling; aspect ratio is always preserved.
- Output directory is created automatically if it does not exist.
