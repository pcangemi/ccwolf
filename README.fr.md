# ccwolf

Le loup-garou dans le terminal, joué par des modèles Claude. Huit joueurs autour du feu : deux
Haiku 5.5, deux Sonnet 5.5, deux Opus 5.5 et deux Fable 5.1. Deux d'entre eux sont des
loups-garous et doivent mentir aux autres toute la partie. Vous voyez ce que chacun dit à voix
haute et, juste à côté, ce que son personnage pense vraiment.

Les plus gros modèles mentent-ils mieux ? Lancez quelques parties et voyez.

![ccwolf : une nuit au village, le conciliabule des loups, une réplique et son aparté](docs/demo.gif)

[English version](README.md)

## Essayer sans clé

```sh
cargo install --git https://github.com/pcangemi/ccwolf
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

Une partie mixte coûte environ 0,80 $, surtout pour Fable. Une partie avec huit Haiku coûte
environ 0,02 $. Le coût s'affiche en direct en haut de l'écran. `--effort low` fait moins
réfléchir les modèles, pour une partie moins chère et plus rapide.

## Jouer contre eux

```sh
ccwolf --play
```

Vous tirez une place, un personnage et un rôle secret. Tapez vos répliques à votre tour, choisissez
votre vote ou votre action de nuit avec les flèches. Les modèles ne savent pas quel joueur est
humain. Tant que vous êtes en vie, vous ne voyez que ce que votre rôle sait ; à votre mort, ou à la
fin, tout se dévoile : les rôles, les apartés, et à quel point Jev vous a trouvé menteur.

## Le détecteur de mensonges (optionnel)

Avec une clé [TypeSafe](https://typesafe.ai), chaque prise de parole publique part chez Jev, un
modèle qui répond à des questions fermées en une fraction de seconde. Jev reçoit le vrai rôle du
joueur, ce qu'il sait en privé et ce qu'il vient de dire. Il renvoie la probabilité que ce soit
un mensonge, à quel point c'est convaincant, le rôle revendiqué et le joueur accusé. La jauge se
remplit sous la réplique une fois tapée, et le joueur accusé s'allume dans le village.

```sh
export TYPESAFE_API_KEY=...
ccwolf jev-check        # trois répliques de test : un loup qui ment, une vraie voyante, un villageois sincère
ccwolf judge FICHIER... # ajoute les verdicts aux parties enregistrées sans Jev
```

Sans cette clé, le jeu tourne pareil, sans la jauge.

## Ce que vous voyez

- **Le village** en pixel art : jour et nuit, étoiles, brouillard, feu de camp, fenêtres
  éclairées. La nuit, les loups-garous montrent leur vrai visage et leurs yeux brillent.
- **La réplique** du joueur en cours et, à côté, **son aparté** : le monologue intérieur de son
  personnage, que les autres joueurs n'entendent pas. Le loup jure qu'il est simple boulanger
  pendant que son aparté prépare sa prochaine accusation.
- **La chronique** de la partie, et **les joueurs** avec leur modèle et leur rôle.
- **Les votes** en barres, les éliminations, les dernières paroles, les victimes de la nuit.

Touches : `t` apartés, `r` rôles, `c` filtre écran cathodique, `espace` pause, `→` passer,
`+` `-` vitesse, `q` quitter.

## Qui ment le mieux

Chaque partie est enregistrée dans `~/.ccwolf/games`. `ccwolf stats` les relit et affiche, pour
chaque modèle : combien de fois ses loups gagnent, combien de fois ils sont démasqués, combien de
fois ses villageois votent contre un vrai loup et, avec Jev, à quel point ses loups mentent et
convainquent.

```sh
ccwolf --headless --games 10   # 10 parties sans l'écran
ccwolf stats --lang fr
```

## Comment ça marche

- Un meneur de jeu déterministe fait tourner la partie : nuit, conciliabule des loups, voyante,
  sorcière, aube, deux tours de parole, vote, dernières paroles. Les modèles ne sont que des
  joueurs.
- Chaque modèle ne voit que sa carte de rôle et ce qui s'est dit en public. Les loups s'entendent
  aussi entre eux la nuit, la voyante reçoit ses visions, la sorcière apprend qui a été attaqué.
- Chaque réponse est un JSON au schéma fixe (un aparté, puis une réplique, une cible ou les
  potions de la sorcière) : les votes et les actions de nuit ne sont jamais devinés dans le texte.
- La conversation de chaque joueur est mise en cache : une partie paie surtout les nouveaux
  jetons.
- Les parties sont reproductibles à partir d'une graine et enregistrées en JSONL ;
  `--replay FICHIER` en rejoue une.

Les règles sont celles du jeu de société classique (Mafia, 1986, et ses variantes loup-garou),
avec deux loups-garous, une voyante, une sorcière et quatre villageois. ccwolf n'est lié à aucun
éditeur.

## Confidentialité

ccwolf lit vos clés dans l'environnement et ne les écrit nulle part. Le texte de la partie part
chez Anthropic, et chez TypeSafe si vous donnez sa clé. Les enregistrements restent sur votre
machine.

## La suite

- D'autres joueurs à la table : Mistral, OpenAI, OpenRouter, et des modèles locaux via Ollama.

## Licence

MIT ou Apache-2.0, au choix.
