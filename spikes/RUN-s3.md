# S3 · Dual-audio capture — RUN

**Goal:** open the real mic **and** a BlackHole input at the same time via `cpal`,
capture ~10 s, and write a stereo WAV (L = mic / you, R = BlackHole / remote).
Proves the §4 audio model: two physical streams = free 2-way attribution.

## 1. Install BlackHole (virtual audio device)

BlackHole is a free virtual audio loopback driver. Install the 2-channel build:

```sh
brew install blackhole-2ch
```

(Or download the installer from <https://existential.audio/blackhole/>.)

> ⚠️ **REBOOT after installing.** The macOS audio HAL does not reliably pick up
> the new driver until you restart — `cpal` will not see "BlackHole 2ch" as an
> input device until then. If the spike says "no BlackHole input device found",
> reboot first.

## 2. Create a Multi-Output Device

So that remote audio is **heard by you** *and* **copied to BlackHole** at the same
time:

1. Open **Audio MIDI Setup** (`/Applications/Utilities/Audio MIDI Setup.app`).
2. Click **+** (bottom-left) → **Create Multi-Output Device**.
3. Check **both** your normal output (e.g. "MacBook Pro Speakers" or your
   headphones) **and** "BlackHole 2ch".
4. Set this Multi-Output Device as the system output (menu bar volume control, or
   System Settings → Sound → Output).

Now any app's audio (a Zoom/Meet call, or just a YouTube tab) plays through your
speakers **and** lands on BlackHole's input, where `cpal` can capture it as the
"remote" side.

## 3. Grant mic permission

The first run triggers the macOS microphone-permission prompt (for the terminal /
your IDE). Allow it, then re-run.

## 4. Run

```sh
# default: 10 seconds, writes dual_capture.wav
cargo run --release --bin s3_dual_audio

# custom duration and output path
cargo run --release --bin s3_dual_audio -- 15 my_capture.wav
```

The spike prints the two devices it opened, e.g.:

```
mic  (L): MacBook Pro Microphone
black(R): BlackHole 2ch
capturing 10s...
```

**While it captures:** talk into your mic (→ left channel) and play a YouTube tab
(routed through the Multi-Output Device → right channel).

## 5. Verify attribution

Play back the WAV and confirm the split:

```sh
afplay dual_capture.wav        # or open in any player with L/R panning
```

- Your voice → **left** only.
- The remote audio (YouTube tab) → **right** only.

That confirms mic → L = "you" and BlackHole → R = "remote", with no diarization.

## Troubleshooting

- **"no BlackHole input device found"** → BlackHole not installed, or you didn't
  reboot. Reboot. The error lists the input devices `cpal` *can* see.
- **Right channel silent** → the Multi-Output Device isn't the system output, or
  doesn't include BlackHole. Re-check step 2.
- **Left channel silent** → mic permission denied, or the wrong default input.
  Check System Settings → Sound → Input.
