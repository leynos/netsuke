# Hello World Example

A minimal Netsuke example that demonstrates:

- Basic manifest structure with `netsuke_version` and `targets`
- Using variables with Jinja templating
- File transformation (text to uppercase)
- Default targets

## Usage

From this directory, run:

```sh
netsuke
```

This builds the default targets (`output.txt` and `greeting.txt`).

## Expected Output

After running `netsuke`:

- `output.txt` contains the uppercase version of `input.txt`
- `greeting.txt` contains "Hello from Netsuke!"

## Files

- `Netsukefile` — The build manifest
- `input.txt` — Sample input file for transformation
- `README.md` — This file
