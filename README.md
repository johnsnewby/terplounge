# Terplounge, a tool to practise oral interpretation

This repository contains the source code to Terplounge, a tool to allow solitary practise of simultaneous interpretation.

## Overview

The basic idea is that the user will listen to some spoken audio in one language (called the 'source' language), translate it into another ('target') language, and speak the translation out loud. A speech-to-text engine will transcribe the target audio, and at the end the user will be shown a comparison of the pre-existing translation, and their own.

## The architecture

![Architecture diagram](doc/img/architecture.png "The Terplounge architecture")

Terplounge is designed to be usable as a hosted product, or on your machine. Its core is a Rust program which contains a version of the Whisper speech-to-text engine which is optimised to run on normal computers. Users connect to this program, which contains a web server, and stream audio to it, which is converted to text and stored in a session (which is not persisted--i.e. it is gone when the program terminates). The program ships with a minimal interface contained within itself, which exposes the basic features of terplounge.

However the idea is that these simple components are just the start of what can be done. By building a dynamic web site around these core services a rich environment can be created.

By default the system will use whisper.cpp for its transcription services, running locally on the CPU. If you have a beefier machine elsewhere you can additionally point it at a remote WhisperX server with `WHISPER_SERVER`; both backends then pull from the same job queue.

### How audio becomes a transcript

1. The browser opens a websocket to `/chat` and streams raw **float32 little-endian** mono samples at whatever rate its audio hardware runs at.
2. The server buffers them and looks for a natural cut: at least 15 seconds of audio, then a 200 ms window quiet enough to count as silence. It splits in the middle of that silence.
3. Each chunk is resampled to the 16 kHz whisper wants and pushed onto a queue that a pool of whisper workers pulls from.
4. Whisper returns one or more segments per chunk. Each is stored against the session and pushed back down the websocket as JSON. Because several workers run at once, segments arrive **out of order** — the client reassembles them by sequence and segment number.
5. When the client is done it POSTs `/close/:uuid`. In-flight jobs keep running; once the last one lands, the session writes its transcript out and shuts down.

Sessions live in memory only. They are gone when the process exits.

## Project structure

Only the `terplounge/` directory (this one) is tracked in git.

| Path | What it is |
|---|---|
| `server/` | The Rust server. This is the product. |
| `server/src/session.rs` | Session state, the audio buffer, and the shutdown handshake |
| `server/src/translate.rs` | Silence detection and resampling |
| `server/src/whispercpp.rs` | Local whisper.cpp worker pool |
| `server/src/whisperx.rs` | Optional remote WhisperX worker |
| `server/src/compare.rs` | Word-diff of transcript against the reference translation |
| `server/templates/` | Askama server-rendered pages |
| `client/` | The original vanilla-JS frontend, **compiled into the binary** |
| `scripts/` | Model downloader, raw-audio test client, static file server |
| `models/` | Where whisper models go (gitignored) |
| `test/jfk.raw` | Sample raw audio for testing without a microphone |

Two sibling directories are used at runtime but are **not** part of this git repository, so a fresh clone will not have them:

| Path | What it is |
|---|---|
| `../terplounge-fe/` | React + TypeScript frontend, a work-in-progress replacement for `client/` |
| `../terplounge-assets/` | Practice materials: audio plus one reference translation per language |

### Practice assets

An asset is a directory containing an audio file, one text file per language, and a `metadata.json`:

```json
{
  "name": "One Day in Berlin - John F. Kennedy 1963",
  "url": "https://commons.wikimedia.org/wiki/...",
  "license": "US Govt",
  "audio": "speech.webm",
  "native": "en",
  "transcript": "en.txt",
  "translations": { "de": "de.txt", "fr": "fr.txt", "es": "es.txt" }
}
```

An `assets.json` at the top of the assets directory lists the directory names as a flat JSON array. The frontend reads that, then fetches each `metadata.json` to build the source/target language pickers.

## Prerequisites

- A **stable** Rust toolchain. The crate is edition 2024, so you need Rust 1.85 or newer. (It used to require nightly for let-chains; it no longer does.)
- A C compiler and `libclang`. `whisper-rs-sys` compiles whisper.cpp and runs `bindgen` over its headers, so on Debian/Ubuntu that means `build-essential` and `libclang-dev`.
- Node 18+, only if you want the React frontend.

## Getting a whisper model

Transcription needs a ggml model. The model is loaded lazily on the first transcription request, so the server will start and serve pages without one — you just will not get any text back.

```
./scripts/download-ggml-model.sh medium
```

This writes to `models/ggml-medium.bin`, which is where the server looks. Run the script with no arguments to list the available models:

```
tiny.en, tiny, tiny-q5_1, tiny.en-q5_1,
base.en, base, base-q5_1, base.en-q5_1,
small.en, small.en-tdrz, small, small-q5_1, small.en-q5_1,
medium, medium.en, medium-q5_0, medium.en-q5_0,
large-v1, large, large-q5_0
```

`medium` is a 1.5 GB download and is the default. The smaller models are much faster and noticeably worse.

## Building and running

### The server on its own

```
cd server
cargo build
ASSETS_DIR=../../terplounge-assets cargo run
```

It listens on <http://127.0.0.1:3030>. `ASSETS_DIR` is worth setting: it defaults to `../assets`, which does not exist in this layout, and without it the practice material picker comes up empty.

The bundled vanilla frontend is served straight out of the binary:

- <http://127.0.0.1:3030/transcribe.html> — freeform transcription
- <http://127.0.0.1:3030/choose.html> — pick source and target languages, then a practice text
- <http://127.0.0.1:3030/websocket.html> — bare-bones page for smoke-testing

Because `client/` is embedded into the executable at compile time, **editing anything under `client/` needs a `cargo build` to take effect.** Reloading the page will not do it.

### With the React frontend, in development

Two processes. The Vite dev server proxies API and websocket traffic through to the Rust server, so you get hot reloading on the frontend while talking to the real backend.

```
# terminal 1
cd server
ASSETS_DIR=../../terplounge-assets cargo run

# terminal 2
cd ../terplounge-fe
npm install
npm run dev
```

Then open <http://localhost:5173>.

### With the React frontend, as one process

Build the frontend to static files and let the Rust server serve them:

```
cd ../terplounge-fe
npm run build

cd ../terplounge/server
ASSETS_DIR=../../terplounge-assets \
REACT_BUILD_DIR=../../terplounge-fe/dist \
cargo run
```

Everything is then on <http://127.0.0.1:3030>.

Note that `/` serves the server's own session-list page, not the React app — that route is claimed earlier in the filter chain. The React app is reachable at its own routes, e.g. `/transcribe` and `/choose`. Deep links like `/practice/<asset>/<lang>` fall back to the SPA's `index.html`, but only for requests that ask for HTML, so a mistyped API path still returns an error rather than a page.

## Environment variables

Read from `server/.env` (see `server/.env.sample`) or the process environment.

| Variable | Default | Purpose |
|---|---|---|
| `WHISPER_MODEL` | `medium` | Model basename, loaded from `models/ggml-<name>.bin` |
| `WHISPER_PROCESSES` | CPU count / 4 | Number of concurrent local whisper workers |
| `WHISPER_SERVER` | unset | If set, also run a remote WhisperX worker against this URL |
| `ASSETS_DIR` | `../assets` | Practice materials |
| `RECORDINGS_DIR` | unset | Where per-session WAV, transcript and metadata are written. **If unset, none of that is saved.** |
| `LISTEN` | `127.0.0.1:3030` | Bind address |
| `REACT_BUILD_DIR` | `../terplounge-fe/dist` | Built React app to serve. Note the default resolves to `terplounge/terplounge-fe/dist`, which is not where the frontend lives — set it to `../../terplounge-fe/dist`. |
| `RUST_LOG` | — | Standard `env_logger` filter, e.g. `RUST_LOG=debug` |
| `RUST_BACKTRACE` | — | Standard |

## HTTP endpoints

| Route | Purpose |
|---|---|
| `GET /chat` | Websocket: audio in, transcription segments out |
| `POST /close/:uuid` | Signal that no more audio is coming |
| `GET /status/:uuid` | Session progress as JSON |
| `GET /transcript/:uuid` | The transcript as plain text |
| `GET /compare/:asset/:uuid/:lang` | Diff page against the reference translation |
| `GET /changes/:asset/:uuid/:lang` | The diff as JSON |
| `GET /practice/:asset/:lang` | Server-rendered practice page |
| `GET /serve_resource/:asset` | An asset's audio file |
| `GET /assets/*`, `GET /recordings/*` | Static directories |
| `GET /` | List of active sessions |

Progress is reported as `transcription_completed_count` out of `transcription_job_count`. There is no sequence-number field; asking for one gets you `undefined`.

## Testing

There are no automated tests. Verification is manual.

**In a browser.** Open `/websocket.html`, choose a microphone and hit start. After about fifteen seconds of speech the first chunk is sent and text should start coming back.

**Without a microphone.** Pipe a raw float32 file straight into the websocket. This needs [`websocat`](https://github.com/vi/websocat):

```
./scripts/send-raw.bash test/jfk.raw en
```

The sample rate in that script must match the file. `test/jfk.raw` is 48 kHz. Then check the result:

```
curl localhost:3030/status/<uuid>
curl localhost:3030/transcript/<uuid>
```

The `uuid` comes back as the first message on the websocket.

**Frontend only.** `scripts/local-server.py` serves `client/` over Flask without needing the Rust server, which is handy for pure markup and CSS work:

```
pip install -r scripts/requirements.txt
python scripts/local-server.py
```

## License

See [LICENSE](LICENSE).
