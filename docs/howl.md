# The howl

`howl.flac` is a gray wolf recorded at Denali National Park and Preserve, Alaska, by the US
National Park Service. Credit: NPS. As a work of the US federal government it is in the public
domain.

Source: https://www.nps.gov/subjects/sound/sounds-wolf.htm
File: https://www.nps.gov/nps-audiovideo/legacy/mp3/nri/avElement/nri-WolfDENA.mp3
Fetched on 2026-10-20.

The file keeps the first howl only, with the hum and rumble under 280 Hz removed:

```
ffmpeg -i nri-WolfDENA.mp3 -af "atrim=start=0.22:end=4.62,asetpts=PTS-STARTPTS,pan=mono|c0=0.5*c0+0.5*c1,highpass=f=280:poles=2,highpass=f=280:poles=2,lowpass=f=7000,afade=t=in:d=0.04,afade=t=out:st=3.75:d=0.65" -ar 44100 -sample_fmt s16 howl.flac
```

The promo storyboards point at it with `"howl": "howl.flac"`. Without that line, the howl is
synthesized like the other sound effects.
