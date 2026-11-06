# gh-inbox

`gh-inbox` is a `gh` extension that turns GitHub notifications into a focused inbox for pull request review.

The project is still under active development.

## Development

Install frontend dependencies:

```bash
npm install
```

Start the app in development mode:

```bash
cargo run
```

This starts the Rust backend, launches the Vite dev server in debug builds, and opens the app in your browser.

Options (also available as `gh inbox <options>`):

- `-p, --port <PORT>`: listen on a fixed port instead of a random one (also `GH_INBOX_PORT`)
- `--no-open`: do not open the browser on startup

```bash
cargo run -- --port 8080 --no-open
```

Run tests:

```bash
cargo test
npm test
```
