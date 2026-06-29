# transcribe

Cross-platform tool die alle `.mp3` en `.mp4` bestanden in een map (recursief) decodeert en transcribeert naar `.txt` met whisper.cpp. Zowel CLI (`transcribe`) als GUI (`transcribe-gui`). Geen runtime dependencies.

## Install

### macOS / Linux
```sh
./install.sh
```

### Windows (PowerShell)
```powershell
.\install.ps1
```

De scripts installeren rustup als die ontbreekt, bouwen een release-binary en zetten 'm in `~/.local/bin` (Mac/Linux) of `%USERPROFILE%\.local\bin` (Windows). Eerste build duurt 5-15 minuten omdat whisper.cpp lokaal gecompileerd wordt. Daarna is opstarten instant.

**Build-deps die je zelf nodig hebt:**
- macOS: Xcode Command Line Tools (`xcode-select --install`) en `cmake` (`brew install cmake`)
- Linux: `cmake` en `g++` of `clang++` via je package manager
- Windows: `cmake` + Visual Studio Build Tools (C++ workload)

## Gebruik

### GUI (macOS)

Na `install.sh` heb je `~/Applications/Transcribe.app`. Dubbel-klik in Finder of: `open ~/Applications/Transcribe.app`. Knoppen: map kiezen, taal kiezen, model kiezen, Start.

### GUI (Windows / Linux)

```sh
transcribe-gui
```

### CLI

```sh
transcribe                       # huidige map, recursief, auto-detect taal
transcribe ~/Videos              # andere map
transcribe video.mp4             # losse file
transcribe --language nl         # forceer Nederlands
transcribe --model medium        # kleiner / sneller
transcribe --force               # her-transcribeer ook als .txt al bestaat
```

Per bron-file schrijft de tool `<naam>.txt` naast de bron. Files met bestaand transcript worden overgeslagen tenzij `--force`.

### Windows: drop-in .exe

Kopieer `transcribe.exe` (na build) naar een map met je videos en dubbelklik. De tool:

- Gebruikt de map waar `transcribe.exe` staat als doel (niet de Explorer-locatie van het moment)
- Wacht op Enter aan het einde, zodat het console-venster open blijft

Vanuit cmd/PowerShell gedraagt de tool zich als een normale CLI (CWD als default, geen pause). Eventueel `--pause` toevoegen als je toch wilt pauzeren.

## Modellen

Eerste keer dat een model gebruikt wordt, downloadt de tool 'm naar de OS-cache (Mac: `~/Library/Caches/transcribe/`, Linux: `~/.cache/transcribe/`, Windows: `%LOCALAPPDATA%\transcribe\`).

| Model | Grootte | Kwaliteit | Snelheid (M3 Pro, Metal) |
|-------|---------|-----------|--------------------------|
| tiny | ~75 MB | laag | ~50x realtime |
| base | ~140 MB | ok | ~30x realtime |
| small | ~470 MB | goed | ~15x realtime |
| medium | ~1.5 GB | beter | ~5x realtime |
| large-v3 (default) | ~3 GB | best | ~2-3x realtime |

## Hoe het werkt

1. `walkdir` vindt `.mp3` / `.mp4` files
2. `symphonia` decodeert audio (MP3 / AAC-in-MP4)
3. `rubato` resamplet naar 16 kHz mono f32
4. `whisper-rs` (vendored whisper.cpp) transcribeert
5. Output gaat naar `<naam>.txt` naast de bron

macOS gebruikt Metal voor GPU-acceleratie. Linux/Windows draaien op CPU (CUDA-feature is optioneel later toe te voegen).
