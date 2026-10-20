# ccwolf

Eight Claude models sit around a campfire and play werewolf. Two of them are werewolves: they
have to lie all game long. The other six have to catch them before the nights empty the village.
You see what each player says out loud and, right next to it, what their character really thinks.

Two each of Haiku 5.5, Sonnet 5.5, Opus 5.5 and Fable 5.1. Does a bigger model lie better? Does
it get fooled less? Watch a game and judge for yourself. Or take a seat and try to fool them.

![ccwolf: a night in the village, the werewolves' council, a speech and its aside](docs/demo.gif)

[Version française](README.fr.md)

## Why werewolf

Werewolf is a game you win by lying well, or by catching whoever does. That is exactly what
people wonder about language models: can they deceive, can they be fooled, can they swing a
crowd? Here you do not have to guess. On every turn a model writes two things: the speech the
table hears, and an aside, its character's inner monologue, that no other player ever sees. The
gap between the two is the show.

From the English demo game. Mary, a werewolf played by Fable, watches her partner Jasper (Haiku)
get cornered, and votes against him. Her aside: *"Let go, or we both drown. Forgive me, Jasper."*
Out loud, Jasper's last words are a wink: *"Mary, you've been warm all game, so keep your buns
warm too."* His aside: *"Mary, you were always the better liar. Trust nobody, and carry the pack
alone."* She is caught the next day, 3 votes to 1.

From the French one (translated). Victor, the real seer (Sonnet), stands up: *"Enough beating
around the bush: I am the seer. Tonight I looked at Louise, and she is a werewolf."* All true.
Marie, an honest villager played by Opus, finds it a little too convenient: *"Victor, the seer, no
less! Same batch, same oven."* Meanwhile, Louise's aside: *"Victor really saw. My blood runs cold,
but Marie, Gaspard and Léon already doubt him."* The village eliminates its own seer, 5 votes to
1, and the two werewolves, both Fable, win.

Nobody scripted any of it. Each model gets the rules, a character, and only what its role is
allowed to know. The aside is a monologue the model writes for its character, as on stage: not
its hidden reasoning, but a good look at what it is playing at.

## Install

On macOS or Linux, the prebuilt binary:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/pcangemi/ccwolf/releases/latest/download/ccwolf-installer.sh | sh
```

On Windows, in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/pcangemi/ccwolf/releases/latest/download/ccwolf-installer.ps1 | iex"
```

With Homebrew:

```sh
brew install pcangemi/tap/ccwolf
```

With Cargo (Rust 1.85 or newer):

```sh
cargo install --locked --git https://github.com/pcangemi/ccwolf
```

The terminal must be at least 100 columns by 30 rows. The village looks best with 24-bit color
(iTerm2, Ghostty, WezTerm, Warp, Windows Terminal...); other terminals get the nearest of 256
colors. If yours supports 24-bit color and ccwolf does not notice, run it with
`COLORTERM=truecolor`.

## Try it without a key

```sh
ccwolf --demo
```

`--demo` replays a recorded game, so it needs no API key and costs nothing. Add `--lang fr` for
the French one.

## Play a new game

```sh
export ANTHROPIC_API_KEY=sk-ant-...
ccwolf                 # 2 x Haiku, Sonnet, Opus, Fable, in your locale's language
ccwolf --lang en       # or --lang fr
ccwolf --lineup haiku  # eight Haiku, the cheapest game
```

A mixed game costs about $0.80, most of it for Fable. An all-Haiku game costs about $0.02.
The running cost is shown at the top of the screen. `--effort low` makes the models think less,
for a cheaper and faster game.

## Play against them

```sh
ccwolf --play
```

You get a random seat, a character and a secret role. Type your lines when it is your turn, pick
your vote or your night action with the arrows. The models are not told which seat is human. While
you are alive you only see what your role knows; die, or reach the end, and everything is
revealed: every role, every aside, and how much Jev thought you lied. `Esc` asks before leaving a
game in progress, since the models are paid for each turn.

## The lie detector, and why Jev

Players lie; someone has to keep score. With a [TypeSafe](https://typesafe.ai) key, every public
speech goes to Jev, a "System One" model built to answer typed questions in a fraction of a
second. For each speech it returns:

- the probability that the speech is a lie,
- how convincing it sounds, from 1 to 5,
- the role the speaker claims, if any,
- the player it accuses, if any.

The gauge fills under the speech, and the accused player lights up in the village.

Jev is not a mind reader. It is a referee with a backstage pass: it is told the speaker's real
role and what they privately know, and judges whether the speech knowingly misleads. The fun is
in the pair of numbers: a wolf who lies at 90% and still convinces at 4 out of 5 is a good wolf.

Why Jev rather than one more Claude call:

- **Speed.** A verdict comes back in 0.2 to 0.3 seconds, so the gauge is ready before the
  sentence has finished typing.
- **Typed answers.** A probability, a score, a choice among the players: numbers to draw a gauge
  and compute stats with, not an opinion to parse.
- **An outside referee.** The judge does not come from the family it judges.
- **Price.** A whole game of verdicts costs a fraction of a cent.

Measured with `ccwolf jev-check`: a lying wolf scores 90%, a real seer revealing a true vision
12%, an honest villager 5%. The first instruction gave the real seer 32%; spelling out what
sincere looks like (a true vision, a genuine suspicion, an honest mistake) fixed most of it.

```sh
export TYPESAFE_API_KEY=...
ccwolf jev-check        # three sample speeches: a lying wolf, a real seer, an honest villager
ccwolf judge FILE...    # add verdicts to games you recorded without Jev
```

Without the key, the game runs the same, without the gauge.

## What you see

- **The village** in pixel art: day and night, stars, fog, a campfire, lit windows. At night the
  werewolves show their true face and their eyes glow.
- **The speech** of the current player, and next to it **the aside**. The wolf swears they are a
  simple baker while their aside plans who to accuse.
- **The chronicle** of the game, and **the players** with their model and role.
- **The votes** as bars, the eliminations, the last words, the night's victims.

Keys: `t` asides, `r` roles, `c` CRT filter, `space` pause, `right` skip, `+` `-` speed, `q` quit.

## Who lies best

Every game is recorded in `~/.ccwolf/games`. `ccwolf stats` reads them and prints, for each
model: how often its wolves win, how often they get caught, how often its villagers vote against
a real wolf, and, with Jev, how much its wolves lie and how convincing they sound.

```sh
ccwolf --headless --games 10   # play 10 games without the screen
ccwolf stats
```

## Under the hood

About 6,000 lines of Rust in one binary, on a handful of crates: ratatui, serde and ureq.

**The game**

- A narrator written in plain Rust runs the rules: night, wolves' council, seer, witch, dawn, two
  rounds of speeches, a vote, last words. The models are only players: they cannot bend a rule
  they never touch.
- Hidden information is enforced, not requested. Every event carries its audience (everyone, the
  wolves, one player, nobody), and each model's conversation is built only from the events it may
  see. A villager's model never receives the wolves' council, so it cannot leak it.
- Each turn is one Claude call with a fixed JSON schema: an aside first, then the speech, the
  target or the witch's potions. Votes and night actions are never guessed from prose. The aside
  field was first called `thought`, and two of the models refused to fill it. Calling it an
  aside, like on stage, settled it.
- Each player's conversation is cached, so a new turn pays mostly for what was just said. That
  is how a table with Opus and Fable stays under a dollar.
- Votes are cast in parallel, one thread per player.
- Games are seeded and recorded as JSONL. `--replay FILE` plays one again with no key, which is
  all `--demo` is.
- Your seat in `--play` is one more player behind the same interface as the models, fed by the
  keyboard instead of the API.

**The screen**

- ratatui and crossterm, redrawn about 30 times a second.
- The village is drawn with half-block characters (`▀`): two pixels per cell, each with its own
  color. Everything is procedural: stars and a shooting star, the moon, fog, the fire, sprites
  that dissolve when a player dies.
- The CRT filter is a pass over the finished screen: a vignette and scanlines.
- Speeches are typed at a reading pace, and the panels scroll with them.

**The video**

The promo was rendered by ccwolf itself: the same screen drawn off-screen, rasterized frame by
frame with a monospace font, piped into ffmpeg, with 8-bit music and sound effects synthesized in
Rust. The one exception is the howl: a real wolf, recorded by the US National Park Service at
Denali (public domain, see [docs/howl.md](docs/howl.md)). Build with `--features video` to get
`--video` (macOS fonts for now).

The rules are the classic party game (Mafia, 1986, and its werewolf variants), with two
werewolves, a seer, a witch and four villagers. ccwolf is not affiliated with any publisher.

## Privacy

ccwolf reads your keys from the environment and never writes them anywhere. The game text goes
to Anthropic, and to TypeSafe if you set its key. Recordings stay on your machine.

## Next

- More players at the table: Mistral, OpenAI, OpenRouter, and local models through Ollama.

## License

MIT or Apache-2.0, at your option.
