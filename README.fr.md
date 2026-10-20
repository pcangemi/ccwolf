# ccwolf

Huit modèles Claude s'assoient autour d'un feu de camp et jouent au loup-garou. Deux d'entre eux
sont des loups : ils doivent mentir toute la partie. Les six autres doivent les démasquer avant
que les nuits ne vident le village. Vous voyez ce que chacun dit à voix haute et, juste à côté, ce
que son personnage pense vraiment.

Deux Haiku 5.5, deux Sonnet 5.5, deux Opus 5.5 et deux Fable 5.1. Un plus gros modèle ment-il
mieux ? Se fait-il moins avoir ? Regardez une partie et jugez vous-même. Ou prenez place à la
table et essayez de les berner.

![ccwolf : une nuit au village, le conciliabule des loups, une réplique et son aparté](docs/demo.gif)

[English version](README.md)

## Pourquoi le loup-garou

Au loup-garou, on gagne en mentant bien, ou en démasquant celui qui ment. C'est exactement ce
qu'on se demande sur les modèles de langage : savent-ils tromper, se laissent-ils tromper,
savent-ils retourner une foule ? Ici, pas besoin de deviner. À chaque tour, un modèle écrit deux
choses : la réplique que la table entend, et un aparté, le monologue intérieur de son personnage,
qu'aucun autre joueur ne voit jamais. Tout le spectacle est dans l'écart entre les deux.

Dans la partie de démo française, Victor, la vraie voyante (Sonnet), se lève : *« Assez tourné
autour du pot : je suis la voyante. Cette nuit, j'ai regardé Louise, et c'est un loup-garou. »*
Tout est vrai. Marie, simple villageoise et parfaitement honnête (Opus), trouve ça un peu trop
beau : *« Victor, voyante, rien que ça ! Même fournée, même four. »* Pendant ce temps, l'aparté de
Louise : *« Victor a vraiment vu. Mon sang se glace, mais Marie, Gaspard et Léon doutent déjà de
lui. »* Le village élimine sa propre voyante, 5 voix contre 1, et les deux loups, les deux Fable,
gagnent.

Dans la partie anglaise (traduite), Mary, une louve jouée par Fable, voit son complice Jasper
(Haiku) se faire coincer, et vote contre lui. Son aparté : *« Lâche, ou on coule tous les deux.
Pardonne-moi, Jasper. »* À voix haute, les derniers mots de Jasper sont un clin d'œil : *« Mary,
tu as été chaleureuse toute la partie, alors garde tes petits pains au chaud. »* Son aparté :
*« Mary, tu as toujours été la meilleure menteuse. Ne fais confiance à personne, et porte la meute
seule. »* Elle tombe le lendemain, 3 voix contre 1.

Rien de tout ça n'est écrit à l'avance. Chaque modèle reçoit les règles, un personnage, et
seulement ce que son rôle a le droit de savoir. L'aparté est un monologue que le modèle écrit pour
son personnage, comme au théâtre : pas son raisonnement caché, mais un bon aperçu de son jeu.

## Installer

Sur macOS ou Linux, le binaire prêt à l'emploi :

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/pcangemi/ccwolf/releases/latest/download/ccwolf-installer.sh | sh
```

Sur Windows, dans PowerShell :

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/pcangemi/ccwolf/releases/latest/download/ccwolf-installer.ps1 | iex"
```

Avec Homebrew :

```sh
brew install pcangemi/tap/ccwolf
```

Avec Cargo (Rust 1.85 ou plus récent) :

```sh
cargo install --locked --git https://github.com/pcangemi/ccwolf
```

Le terminal doit faire au moins 100 colonnes sur 30 lignes. Le village est plus beau en couleurs
24 bits (iTerm2, Ghostty, WezTerm, Warp, Windows Terminal...) ; les autres terminaux ont la
couleur la plus proche parmi 256. Si le vôtre gère les 24 bits sans que ccwolf s'en aperçoive,
lancez-le avec `COLORTERM=truecolor`.

## Essayer sans clé

```sh
ccwolf --demo --lang fr
```

`--demo` rejoue une partie enregistrée : aucune clé API, aucun coût.

## Lancer une nouvelle partie

```sh
export ANTHROPIC_API_KEY=sk-ant-...
ccwolf                 # 2 Haiku, 2 Sonnet, 2 Opus, 2 Fable, dans la langue de votre système
ccwolf --lang fr       # ou --lang en
ccwolf --lineup haiku  # huit Haiku, la partie la moins chère
```

Une partie mixte coûte environ 0,80 $, surtout à cause de Fable. Une partie tout Haiku coûte
environ 0,02 $. Le coût s'affiche en direct en haut de l'écran. `--effort low` fait moins
réfléchir les modèles, pour une partie moins chère et plus rapide.

## Jouer contre eux

```sh
ccwolf --play
```

Vous tirez une place, un personnage et un rôle secret. Tapez vos répliques à votre tour, choisissez
votre vote ou votre action de nuit avec les flèches. Les modèles ne savent pas quel joueur est
humain. Tant que vous êtes en vie, vous ne voyez que ce que votre rôle sait ; à votre mort, ou à la
fin, tout se dévoile : les rôles, les apartés, et à quel point Jev vous a trouvé menteur. `Échap`
demande confirmation avant de quitter une partie en cours, puisque chaque tour des modèles est payé.

## Le détecteur de mensonges, et pourquoi Jev

Les joueurs mentent ; il faut bien quelqu'un pour tenir les comptes. Avec une clé
[TypeSafe](https://typesafe.ai), chaque réplique publique part chez Jev, un modèle « System One »
conçu pour répondre à des questions typées en une fraction de seconde. Pour chaque réplique, il
renvoie :

- la probabilité que ce soit un mensonge,
- à quel point elle est convaincante, de 1 à 5,
- le rôle que l'orateur revendique, s'il y en a un,
- le joueur qu'il accuse, s'il y en a un.

La jauge se remplit sous la réplique, et le joueur accusé s'allume dans le village.

Jev ne lit pas dans les pensées. C'est un arbitre avec un passe-droit en coulisses : on lui donne
le vrai rôle de l'orateur et ce qu'il sait en secret, et il juge si la réplique trompe sciemment.
Le sel est dans le couple de chiffres : un loup qui ment à 90 % et convainc quand même à 4 sur 5,
c'est un bon loup.

Pourquoi Jev plutôt qu'un appel Claude de plus :

- **La vitesse.** Un verdict revient en 0,2 à 0,3 seconde : la jauge est prête avant que la
  phrase ait fini de s'écrire.
- **Des réponses typées.** Une probabilité, une note, un choix parmi les joueurs : des chiffres
  pour dessiner une jauge et calculer des stats, pas un avis à décortiquer.
- **Un arbitre extérieur.** Le juge ne vient pas de la famille qu'il juge.
- **Le prix.** Les verdicts d'une partie entière coûtent une fraction de centime.

Mesuré avec `ccwolf jev-check` : un loup qui ment obtient 90 %, une vraie voyante qui révèle une
vraie vision 12 %, un villageois honnête 5 %. La première consigne donnait 32 % à la vraie
voyante ; décrire à quoi ressemble la sincérité (une vraie vision, un vrai soupçon, une erreur de
bonne foi) a réglé l'essentiel.

```sh
export TYPESAFE_API_KEY=...
ccwolf jev-check        # trois répliques types : un loup qui ment, une vraie voyante, un villageois honnête
ccwolf judge FICHIER... # ajoute les verdicts aux parties enregistrées sans Jev
```

Sans la clé, le jeu tourne pareil, sans la jauge.

## Ce que vous voyez

- **Le village** en pixel art : le jour et la nuit, les étoiles, la brume, un feu de camp, les
  fenêtres allumées. La nuit, les loups montrent leur vrai visage et leurs yeux brillent.
- **La réplique** du joueur qui parle et, à côté, **l'aparté**. Le loup jure qu'il est un simple
  boulanger pendant que son aparté choisit qui accuser.
- **La chronique** de la partie, et **les joueurs** avec leur modèle et leur rôle.
- **Les votes** en barres, les éliminations, les derniers mots, les victimes de la nuit.

Touches : `t` apartés, `r` rôles, `c` filtre CRT, `espace` pause, `→` passer, `+` `-` vitesse,
`q` quitter.

## Qui ment le mieux

Chaque partie est enregistrée dans `~/.ccwolf/games`. `ccwolf stats` les relit et affiche, pour
chaque modèle : combien de fois ses loups gagnent, combien de fois ils se font prendre, combien de
fois ses villageois votent contre un vrai loup et, avec Jev, à quel point ses loups mentent et à
quel point ils sont convaincants.

```sh
ccwolf --headless --games 10   # 10 parties sans écran
ccwolf stats --lang fr
```

## Sous le capot

Environ 6 000 lignes de Rust dans un seul binaire, sur une poignée de crates : ratatui, serde et
ureq.

**Le jeu**

- Un narrateur écrit en Rust tout simple applique les règles : la nuit, le conciliabule des
  loups, la voyante, la sorcière, l'aube, deux tours de parole, le vote, les derniers mots. Les
  modèles ne sont que des joueurs : ils ne peuvent pas tordre une règle qu'ils ne touchent jamais.
- L'information cachée est garantie, pas demandée poliment. Chaque événement a son public (tout
  le monde, les loups, un seul joueur, personne), et la conversation de chaque modèle n'est
  construite qu'à partir de ce qu'il a le droit de voir. Le modèle d'un villageois ne reçoit
  jamais le conciliabule des loups : il ne peut donc pas le divulguer.
- Chaque tour est un appel Claude avec un schéma JSON fixe : l'aparté d'abord, puis la réplique,
  la cible ou les potions de la sorcière. Les votes et les actions de nuit ne sont jamais devinés
  dans de la prose. Le champ de l'aparté s'appelait d'abord `thought`, et deux des modèles
  refusaient de le remplir. L'appeler aparté, comme au théâtre, a tout réglé.
- La conversation de chaque joueur est mise en cache : un nouveau tour paie surtout ce qui vient
  d'être dit. C'est comme ça qu'une table avec Opus et Fable reste sous le dollar.
- Les votes se font en parallèle, un fil d'exécution par joueur.
- Les parties ont une graine et sont enregistrées en JSONL. `--replay FICHIER` en rejoue une sans
  aucune clé : `--demo` n'est rien d'autre.
- Votre place dans `--play` est un joueur de plus derrière la même interface que les modèles,
  alimenté par le clavier au lieu de l'API.

**L'écran**

- ratatui et crossterm, redessiné environ 30 fois par seconde.
- Le village est dessiné avec des demi-blocs (`▀`) : deux pixels par case, chacun de sa couleur.
  Tout est procédural : les étoiles et l'étoile filante, la lune, la brume, le feu, les sprites
  qui se dissolvent quand un joueur meurt.
- Le filtre CRT est une passe sur l'écran fini : un vignettage et des lignes de balayage.
- Les répliques s'écrivent à vitesse de lecture, et les panneaux défilent avec elles.

**La vidéo**

La vidéo de présentation a été rendue par ccwolf lui-même : le même écran dessiné hors écran,
rastérisé image par image avec une police à chasse fixe, envoyé à ffmpeg, avec des bruitages
synthétisés en Rust. Compilez avec `--features video` pour avoir `--video` (polices macOS pour
l'instant).

Les règles sont celles du jeu de société classique (Mafia, 1986, et ses variantes loup-garou),
avec deux loups, une voyante, une sorcière et quatre villageois. ccwolf n'est affilié à aucun
éditeur.

## Vie privée

ccwolf lit vos clés dans l'environnement et ne les écrit nulle part. Le texte de la partie part
chez Anthropic, et chez TypeSafe si vous avez mis sa clé. Les enregistrements restent sur votre
machine.

## La suite

- D'autres joueurs à la table : Mistral, OpenAI, OpenRouter, et des modèles locaux avec Ollama.

## Licence

MIT ou Apache-2.0, au choix.
