# PRD — Backline

> Outil de gestion pour collectifs artistiques : de la **négociation d'une date avec un lieu** jusqu'au **dernier post publié**.
>
> **Multi-collectif, sans inscription publique.** **Une seule instance partagée**, déployée sur `backline.betafactory.co`, héberge tous les collectifs. Leur création est manuelle, faite par un administrateur d'instance — il n'y a jamais un déploiement par collectif.
>
> **Nom du produit : Backline** — tout ce qu'il y a derrière la scène pour que le concert existe : le matériel, les rôles, les gens qui portent. Marque compacte : **BCKLN** (le mot sans ses voyelles), pour les logotypes, monogrammes et pseudonymes.
>
> Premier collectif : **Bonsoir Techno**. Groupe de référence pour les tests : **Ramas**, duo dont les deux membres sont également membres du collectif.

---

## 1. Contexte et problème

Le fonctionnement actuel de Bonsoir Techno :

1. Un admin contacte un lieu et obtient **un ensemble de dates possibles**.
2. Il demande aux membres, à la main, qui est disponible pour telle date / tel lieu.
3. S'ensuivent des allers-retours jusqu'à ce qu'une date soit arrêtée et un line-up constitué.
4. L'événement confirmé, il faut prévenir tout le monde, répartir une logistique qui change à chaque fois (son, lumière, transport), produire les visuels, et publier sur les comptes du collectif **et** de chaque groupe, aux bons moments.

Tout est manuel, se perd dans des fils de discussion, et la com part en retard ou pas du tout.

**Ce que l'outil doit produire :** une date arrêtée plus vite, une logistique couverte sans relances humaines, et une com qui sort au bon moment avec les bons visuels.

---

## 2. Périmètre

### Dans le périmètre

| Module                       | Résumé                                                                        |
| ---------------------------- | ----------------------------------------------------------------------------- |
| Instance & collectifs        | Plusieurs collectifs sur une instance, cloisonnés, créés manuellement         |
| Collectifs, groupes, membres | Hiérarchie, appartenances multiples, droits par niveau                        |
| Opportunités & dates         | Lieu, dates candidates, sondage de dispos, arbitrage, conversion en événement |
| Événements                   | Soirées DJ, concerts, résidences, **streams live**                            |
| Logistique                   | Postes à pourvoir libres, volontariat, relances automatiques                  |
| Disponibilités               | Déclaratives, par membre, par date candidate                                  |
| Calendrier                   | Vue unifiée + flux iCal abonnable (Google Calendar, Apple…)                   |
| Charte graphique             | Tokens au niveau collectif + surcharge par groupe                             |
| Studio visuels               | **Éditeur visuel** sans code, ratios verrouillés, gabarits multi-formats      |
| Vidéo                        | Plans et timeline décrits dans l'app, **rendu par CLI sur machines GPU**      |
| Plan de com                  | Timelines instanciées automatiquement, propres à chaque type d'événement      |
| Publication                  | Assignation à un membre, rappel à l'heure, **confirmation manuelle**          |
| Comptes sociaux              | Inventaire des comptes et de **qui y a accès** (jamais les mots de passe)     |
| Fiches techniques            | Par groupe, versionnées, export PDF                                           |
| Press kit                    | Bio, photos, liens, par groupe                                                |
| Notifications                | Bot Telegram interactif + centre de notifications web                         |

### Hors périmètre (explicite)

- **Argent** : cachets, budgets, factures, partage de revenus _(module entier, contaminerait tout le modèle)_
- **Inventaire matériel** (qui possède quel ampli)
- **Setlists** et **billetterie**
- **Publication automatique via API** des réseaux sociaux → §11.2
- **Encodage vidéo sur le serveur** : jamais sur le VPS, le rendu est délégué à des machines locales → §10
- **Génération d'images ou de vidéos par IA** : les médias sont **fournis et téléversés**. La chaîne de génération sera travaillée séparément, hors de cet outil ; l'app se contente d'accueillir les fichiers produits.
- **Stockage des mots de passe** des comptes sociaux → §11.3
- **Inscription libre, facturation, SaaS public** : l'instance est multi-collectif, mais chaque collectif est créé à la main

---

## 3. Utilisateurs, droits et accès

| Rôle                   | Portée           | Peut                                                                                                                                             |
| ---------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Admin d'instance**   | Toute l'instance | Créer des collectifs, créer le premier admin de chaque collectif, exploitation technique                                                         |
| **Admin de collectif** | Un collectif     | Membres, groupes, lieux, opportunités, arbitrage des dates, charte, gabarits, jalons de com                                                      |
| **Admin de groupe**    | Un groupe        | Membres du groupe, charte locale, comptes sociaux, fiche technique, press kit, com des événements portés par le groupe                           |
| **Membre**             | Ses collectifs   | Déclarer ses dispos, se positionner sur un événement, prendre un poste, consulter le calendrier, exécuter et confirmer ses tâches de publication |

Un **compte utilisateur est unique par personne** et peut appartenir à **plusieurs collectifs**, avec un rôle différent dans chacun.

**Il n'existe pas de profil réservé : n'importe quel utilisateur peut recevoir un rôle d'administration**, à n'importe quel niveau. Un admin n'est pas une catégorie de personne, c'est un droit posé sur une appartenance — et retirable. Conséquence pratique : Antoine peut être admin de Bonsoir Techno tout en étant simple membre d'un groupe, et n'importe quel membre peut être promu le jour où il prend en charge la com ou les dates.

### Création de compte — aucune inscription

Un admin crée l'utilisateur (nom, **téléphone**, e-mail optionnel, groupes) et génère un **lien d'invitation à usage unique**. La personne ouvre le lien → le bot Telegram lie son compte. Ensuite :

- **Connexion web** : Telegram Login Widget — aucun mot de passe à gérer pour des dizaines de personnes.
- **Secours** : e-mail + mot de passe, obligatoire pour au moins un admin d'instance, afin de ne jamais dépendre de Telegram pour l'accès d'administration.

### Données personnelles

| Champ                | Usage                                         | Visibilité                                                         |
| -------------------- | --------------------------------------------- | ------------------------------------------------------------------ |
| Nom, pseudo de scène | Partout                                       | Tous les membres du collectif                                      |
| **Téléphone**        | Contact le jour J, feuille de route, urgences | **Admins du collectif + membres du même événement**, jamais public |
| E-mail               | Secours de connexion, envois de documents     | Admins                                                             |
| Identifiant Telegram | Notifications                                 | Système                                                            |

Le téléphone apparaît automatiquement sur la **feuille de route du jour J** (§5.5) — c'est le seul moment où l'on en a réellement besoin.

---

## 4. Modèle de domaine

```
Instance (Backline)
 └── Collectif[]                      (Bonsoir Techno, …)
      ├── Charte (tokens)
      ├── CompteSocial[]              (Instagram, TikTok, YouTube du collectif)
      ├── Appartenance[]              (Utilisateur × Collectif, avec rôle)
      ├── Groupe[]                    (Ramas, … — un seul concept, y compris solo)
      │    ├── Charte (surcharge : logo, couleur d'accent)
      │    ├── CompteSocial[]         (propres au groupe)
      │    ├── MembreGroupe[]         (appartenances multiples, rôle libre : "MAO", "batterie"…)
      │    ├── FicheTechnique[]       (versionnée)
      │    └── PressKit
      ├── Lieu[]
      │    └── Contact[]              (programmateur, régie, production)
      ├── Opportunite[]
      │    ├── DateCandidate[]
      │    └── Sondage → ReponseDispo[]        (membre × date candidate)
      ├── Evenement[]                 (type : soirée DJ | concert | résidence | stream)
      │    ├── Participation[]        (membre/groupe, statut, rôle scène)
      │    ├── PosteLogistique[]      (libellé libre, quantité, assignés)
      │    ├── PlanCom → TachePublication[]
      │    ├── DetailStream?          (si type = stream)
      │    └── Contacts du lieu + FicheTechnique rattachée
      ├── Gabarit[]                   (collectif ou groupe)
      │    └── Declinaison[]          (une par format, ratio verrouillé)
      ├── CompositionVideo[]          (plans, calques, timeline, audio — déclaratif)
      │    └── JobRendu[]             (réclamé et exécuté par une machine GPU)
      └── Asset[]                     (médias téléversés, visuels rendus, vidéos, PDF)

Utilisateur                           (transverse : nom, téléphone, e-mail, Telegram)
MachineDeRendu                        (jeton, propriétaire, capacités GPU, dernier contact)
```

### Règles de domaine

- **Cloisonnement strict par collectif** : toute requête est filtrée par le ou les collectifs de l'utilisateur. C'est une règle d'accès vérifiée au niveau des données, pas seulement de l'interface.
- Un **utilisateur appartient à plusieurs collectifs** et à **plusieurs groupes**, sans contrainte de combinaison.
- Un **groupe est rattaché à un collectif principal** (Ramas → Bonsoir Techno) — ce qui détermine l'héritage de charte. Il peut néanmoins **participer à des événements d'un autre collectif** en tant qu'invité.
- Le **porteur** d'un événement est soit le collectif, soit un groupe. Même objet, champ `porteur`.
- Un **groupe d'une personne** est un groupe comme un autre.
- Un **rôle logistique est une chaîne libre**, saisie à l'événement. Pas de catalogue ; autocomplétion sur les libellés déjà utilisés dans le collectif.
- Une **fiche technique est versionnée** ; l'événement pointe vers la version réellement envoyée au lieu.
- Un **gabarit est conçu sur un format maître** et décliné ; chaque déclinaison a son ratio verrouillé et ses propres ajustements.
- Une **composition vidéo est une description, pas un fichier** : le rendu en est une exécution, reproductible et rejouable.

---

## 5. Flux principal : de la date au concert

### 5.1 Opportunité

Un admin crée une **Opportunité** :

- Lieu (existant ou nouveau) + contact
- Porteur pressenti : le collectif, ou un ou plusieurs groupes
- **N dates candidates** proposées par le lieu (date, horaires, notes)
- Conditions en texte libre _(l'argent est hors périmètre, la note reste utile)_
- Statut : `en discussion` → `sondage ouvert` → `date retenue` → `confirmée` / `abandonnée`

> Les **streams** ne passent pas par une opportunité : pas de lieu à négocier, ils sont créés directement comme événements (§5.6).

### 5.2 Sondage de disponibilités

L'admin ouvre le sondage → le bot envoie à chaque membre concerné **un message par opportunité**, une ligne par date candidate, trois boutons : **✅ dispo / ❔ peut-être / ❌ non**, plus **🎸 je veux jouer**. Saisie identique sur le web.

L'admin obtient une **matrice `membres × dates candidates`** affichant, pour chaque date :

- le nombre de dispos,
- les **groupes complets** (tous leurs membres disponibles) → line-up possible en un coup d'œil,
- les manquants par groupe.

C'est l'écran qui supprime les allers-retours actuels.

**Les disponibilités sont visibles par tous les membres du collectif**, pas seulement par les admins. C'est un choix assumé : chacun voit que la date est en train de se jouer, s'auto-corrige et relance ses collègues sans passer par un admin. Les indisponibilités restent binaires — on ne demande jamais _pourquoi_ quelqu'un n'est pas libre.

**Une disponibilité n'est modifiable que par la personne concernée, admins compris.** Visible de tous, écrite par un seul. Un admin qui reçoit une réponse par téléphone relance la personne via le bot plutôt que de répondre à sa place : sans cette règle, plus personne ne sait si une case « dispo » vient du membre ou d'une supposition, et la matrice d'arbitrage perd toute valeur.

### 5.3 Arbitrage et conversion

L'admin choisit la date, compose le line-up parmi les volontaires, confirme.
→ L'Opportunité devient un **Événement confirmé**, ce qui déclenche automatiquement :

1. L'entrée au calendrier et la mise à jour des flux iCal
2. La notification des participants retenus — et des non-retenus
3. La création des **postes logistiques** (vides)
4. L'instanciation du **plan de com** correspondant au type d'événement
5. Le rattachement des **fiches techniques** des groupes du line-up

### 5.4 Logistique

Postes saisis en texte libre (« transport backline », « lumière », « photo », « bar »), avec une quantité. Les membres se positionnent via le bot ou le web.

**Relances automatiques** sur les postes vacants à **J-14, J-7, J-2**, ciblées sur les membres qui ne se sont encore engagés sur rien pour cet événement.

### 5.5 Feuille de route du jour J

Générée automatiquement, consultable sur le web et envoyée par le bot la veille : horaires (arrivée, balance, ouverture, passage), adresse, contacts du lieu, line-up, **qui tient quel poste avec son téléphone**, lien vers la fiche technique.

### 5.6 Après l'événement

Statut `passé` → déclenche la tâche de com J+2 (remerciements, photos) et archive la fiche.

---

## 6. Types d'événements

Les types sont **paramétrables** par collectif ; chacun porte ses propres champs et sa propre timeline de com.

| Type                  | Spécificités                                                                                                                        | Timeline        |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | --------------- |
| **Soirée DJ électro** | Line-up à créneaux (b2b, warm-up, closing), lieu, jauge                                                                             | §11.1           |
| **Concert**           | Line-up scène, balances, fiche technique obligatoire                                                                                | §11.1           |
| **Résidence**         | **Un seul événement** couvrant toute la plage de dates, présence partielle des membres, objectif de travail, restitution éventuelle | §11.1 (allégée) |
| **Stream live**       | Voir ci-dessous                                                                                                                     | §11.1 (courte)  |

### Résidence

Une résidence est **un événement unique** portant une date de début et une date de fin, pas une série d'événements quotidiens. La plage est **fixe** ; ce sont les présences qui sont souples. Une seule entrée au calendrier sur toute la plage, un seul plan de com.

**Déclaration de présence, volontairement minimale.** La question posée est _« tu viens ? »_, pas _« quand exactement ? »_ :

- **Je viens** — suffisant, sans autre précision. C'est la réponse attendue par défaut.
- **Je ne viens pas**
- **Je ne sais pas encore**

Les **jours précis sont facultatifs** : qui veut peut cocher les journées où il sera là, mais l'app ne le réclame jamais et n'envoie aucune relance pour l'obtenir. Un participant sans jours cochés est un participant valide, affiché comme « présent, dates libres ».

Utilité du détail quand il est renseigné : l'admin voit les jours creux et les jours chargés du studio. C'est un confort, jamais un prérequis.

### Créneaux horaires du line-up

Les horaires de passage sont **facultatifs et indépendants par événement** : parfois ils sont arrêtés très tôt, parfois on ne les connaîtra jamais. Chaque participation porte donc un créneau optionnel, et l'événement porte deux interrupteurs distincts :

- **Créneaux définis** — oui / non / à confirmer
- **Créneaux publiables** — s'ils apparaissent ou non dans les visuels et les textes générés

Tant que les créneaux ne sont pas publiables, les gabarits affichent simplement l'ordre du line-up sans heures ; aucun visuel ne sort avec un horaire provisoire.

### Stream live

Pas de lieu à négocier, mais une logistique et une com propres :

- **Plateformes de diffusion** (multi-diffusion possible) : Twitch, YouTube Live, Instagram Live, TikTok Live — avec l'URL de chaque flux
- Date, **heure de début**, durée prévue
- **Line-up avec créneaux** (qui joue de quand à quand)
- Lieu de captation : studio, salle, chez quelqu'un
- **Besoins techniques** comme postes logistiques : caméras, lumières, régie, connexion, encodage, gestion du chat
- **Lien de replay**, renseigné après coup — il alimente les tâches de com J+1 et J+3

---

## 7. Calendrier et agendas

- Vue **mois / semaine / liste**, filtrable par collectif, par groupe, par statut, par « mes événements »
- Affiche : événements confirmés, dates candidates en arbitrage (en pointillés), indisponibilités déclarées
- **Flux iCal** abonnables, URL à jeton secret : un flux par membre, un par groupe, un par collectif

L'application est **maître**. Le push vers Google Calendar se fait par abonnement iCal — aucun OAuth à faire signer à des dizaines de personnes, aucune synchro bidirectionnelle. Les disponibilités sont **déclarées dans l'app**, jamais lues depuis les agendas personnels.

---

## 8. Charte graphique

Modélisée comme **données éditables**, jamais codée en dur. Une charte par collectif, surcharge partielle par groupe. Écran d'administration dédié.

**Tokens :**

- Couleurs : primaire, secondaire, accent, fond, texte, palette étendue
- Typographies : titre, sous-titre, corps — fichiers de police téléversés, avec graisses
- Logos : collectif et groupes, variantes (couleur, monochrome, fond clair/sombre), zone de sécurité, taille minimale
- Grille et marges par format
- Règles : placement du logo, casse des titres, mentions obligatoires

> **Dépendance d'entrée.** Les chartes de Bonsoir Techno et de Ramas ne sont pas encore formalisées ; elles seront travaillées après ce PRD. Les logos et les polices existent déjà. En attendant, l'app est livrée avec **un jeu de tokens d'exemple sobre** pour que les gabarits soient visibles et testables. Saisir les vraies valeurs **ne demandera aucune modification de code**.

---

## 9. Studio visuels

### 9.1 Un éditeur visuel, pas un éditeur de code

**Principe non négociable : aucune compétence technique n'est requise pour produire un visuel.** L'éditeur est un canevas manipulé à la souris — pas de HTML, pas de CSS, pas de JSON visible.

**Blocs disponibles** — texte, image, vidéo, logo, forme (rectangle, cercle, trait), dégradé, groupe.

**Manipulation** — glisser-déposer, poignées de redimensionnement, rotation, ordre des calques, duplication, verrouillage d'un bloc, groupes.

**Aides à l'alignement** — magnétisme sur la grille, guides d'alignement entre blocs, répartition, centrage, **zones de sécurité affichées** (par exemple les bandeaux d'interface d'une story Instagram, où le texte est masqué par les boutons de l'application).

**Palette de contenu** — un panneau liste les valeurs de la charte (couleurs, polices, logos) et les **champs automatiques** de l'événement (nom, date, heure, lieu, ville, line-up, créneaux, handles). On les dépose sur le canevas ; ils se remplissent tout seuls à la génération.

Seuls les tokens de la charte sont proposés dans les sélecteurs de couleur et de police. **On ne peut pas sortir de la charte par accident** — c'est une contrainte de l'éditeur, pas une règle à respecter de tête.

### 9.2 Formats et ratios verrouillés

Le cadre de travail est **toujours verrouillé au ratio** du format choisi. On ne redimensionne jamais le cadre librement : on choisit un format, et le canevas s'y conforme.

| Plateforme | Format | Ratio | Définition |
|---|---|---|---|
| Instagram | Carré | 1:1 | 1080 × 1080 |
| Instagram | Portrait (recommandé) | 4:5 | 1080 × 1350 |
| Instagram | Story / Reel | 9:16 | 1080 × 1920 |
| TikTok | Vidéo / image | 9:16 | 1080 × 1920 |
| YouTube | Miniature | 16:9 | 1280 × 720 |
| YouTube | Shorts | 9:16 | 1080 × 1920 |
| YouTube | Bannière de chaîne | 16:9 | 2560 × 1440 |
| Facebook | Publication | 1:1 / 16:9 | 1080 × 1080 / 1920 × 1080 |
| Affiche | Impression | A3 / A4 | 300 dpi, fonds perdus |
| Libre | Personnalisé | au choix | ratio verrouillé une fois défini |

Le catalogue est **éditable** : les plateformes changent leurs formats, ça ne doit jamais demander une modification de code.

### 9.3 Un gabarit, plusieurs formats

Un gabarit est conçu sur un **format maître**, puis décliné. À la création d'une déclinaison, l'app propose automatiquement une adaptation (repositionnement, ajustement des tailles de texte) que **l'on corrige à la main** — l'adaptation automatique est un point de départ, jamais un résultat final. Chaque déclinaison mémorise ses propres ajustements sans toucher au maître.

Autres règles :

- **Versionnement** : modifier un gabarit ne casse aucun visuel déjà produit.
- **Duplication** : un gabarit se copie pour servir de base à un autre.
- **Portée** : gabarits partagés au niveau du collectif, ou propres à un groupe.

### 9.4 Le mode le plus fréquent : remplir, pas concevoir

L'usage quotidien n'est pas de dessiner une affiche, c'est de **refaire la même avec d'autres noms**. Depuis un événement, **« générer les visuels »** produit en une passe tous les formats du plan de com, champs automatiques déjà remplis, sans ouvrir l'éditeur. On n'entre dans le canevas que pour créer ou retoucher un gabarit.

### 9.5 Médias

**Téléversement uniquement** : photos, captations, visuels produits ailleurs. Bibliothèque par collectif et par groupe, avec étiquettes, recherche et réutilisation.

La **génération d'images par IA est hors périmètre** : elle sera travaillée avec des outils dédiés, en dehors de l'app, et les fichiers produits seront simplement téléversés. Aucun fournisseur d'IA intégré, aucune clé à gérer, aucun coût variable.

---

## 10. Vidéo : composition dans Backline, rendu distribué

### 10.1 Pourquoi cette séparation

Encoder de la vidéo est l'opération la plus lourde de tout le projet, et un VPS mutualisé est le pire endroit pour le faire : un seul export saturerait la machine qui héberge l'application. En revanche, **les membres ont des machines équipées de GPU**.

Backline **décrit** donc la vidéo ; **une CLI la fabrique**, sur un poste local. Le VPS ne fait jamais d'encodage.

### 10.2 Décrire une vidéo : plans et timeline

L'éditeur vidéo reprend le canevas des visuels et lui ajoute un **axe temporel**.

- **Plans** (scènes) — une suite de plans, chacun avec sa durée et sa transition d'entrée (coupe, fondu, glissement)
- **Calques par plan** — mêmes blocs que pour les images : vidéo, image, texte, logo, forme ; chacun avec un instant d'apparition et de disparition
- **Animations** — jeu restreint et sûr : fondu, translation, zoom lent, apparition de texte. Pas de courbes de Bézier ni d'images clés à la main : le but est qu'Antoine ou Mathieu produisent un teaser correct en dix minutes.
- **Audio** — piste unique, extrait téléversé, points d'entrée et de sortie, fondus
- **Sortie** — format issu du catalogue (§9.2), durée totale, images par seconde
- **Champs automatiques** — date, lieu, line-up, créneaux : identiques aux visuels fixes

Le résultat est une **description déclarative et versionnée** de la vidéo, stockée dans Backline. C'est la recette, pas le plat : elle est légère, relisible, modifiable, et rejouable à l'identique.

**Prévisualisation dans le navigateur** : la timeline se joue dans l'app, sans encodage, pour valider le rythme avant de lancer un rendu. Ce que montre l'aperçu correspond à ce que produira la CLI — même description, même moteur de mise en page.

### 10.3 La CLI de rendu

Un outil en ligne de commande, installé sur les machines des membres qui ont du GPU.

```
backline login                  # associe la machine au compte, via un jeton
backline jobs                   # liste les rendus en attente
backline render                 # prend un job, le rend, renvoie le résultat
backline render --watch         # démon : traite les jobs au fil de l'eau
backline render <id> --preview  # rendu rapide basse définition, pour vérifier
```

Déroulé d'un rendu : la machine **réclame** un job, télécharge la description et les médias sources, compose les plans, encode en **accélération matérielle** quand le GPU le permet (NVENC côté PC, VideoToolbox côté Mac), reverse le fichier fini, marque le job terminé. Progression et erreurs remontent en direct dans l'app.

**Conséquences à assumer, écrites ici pour ne pas les découvrir en route :**

- Si **personne ne fait tourner la CLI**, aucune vidéo ne sort. L'app affiche donc en permanence quelles machines sont connectées, et la tâche de com concernée **reste livrable avec son visuel fixe** — la com ne s'arrête jamais faute de rendu.
- La machine qui rend a **accès aux médias du collectif** le temps du job. Jeton révocable par machine, journal des rendus.
- Un job non réclamé au bout d'un délai **alerte l'admin** plutôt que de rester silencieusement en attente.

## 11. Plan de com et publication

### 10.1 Timelines par type

Instanciées automatiquement à la confirmation d'un événement, **entièrement modifiables** dans les réglages du collectif.

**Concert / soirée DJ**

| Jalon | Contenu                  | Formats               |
| ----- | ------------------------ | --------------------- |
| J-30  | Annonce                  | Post 4:5 + story      |
| J-21  | Focus artiste / line-up  | Post 1:1              |
| J-14  | Extrait audio ou vidéo   | Reel 9:16             |
| J-7   | Rappel + infos pratiques | Post 4:5 + story      |
| J-3   | Teaser court             | 9:16 (Reels + TikTok) |
| J-1   | « Demain »               | Story                 |
| J0    | « Ce soir »              | Story matin + post    |
| J+2   | Remerciements / photos   | Post + story          |

**Résidence** — J-14 annonce · J-7 « on entre en résidence » · pendant : 2 contenus coulisses · J+3 restitution

**Stream live** — cycle court, la réactivité prime

| Jalon       | Contenu                                    | Formats                                     |
| ----------- | ------------------------------------------ | ------------------------------------------- |
| J-7         | Annonce du live (date, heure, plateformes) | Post 4:5 + story                            |
| J-3         | Teaser / invité du stream                  | 9:16                                        |
| J-1         | Rappel                                     | Story                                       |
| J0 − 2 h    | « Ce soir en live »                        | Story                                       |
| J0 − 15 min | **« On est en ligne »** + lien             | Story + notification bot à tous les membres |
| Pendant     | Repartage, lien du flux                    | Story                                       |
| J+1         | **Replay disponible**                      | Post + story avec lien                      |
| J+3         | Extrait court du live                      | 9:16                                        |

Toute tâche naît au statut **`brouillon`** : un humain valide toujours. _(Une affiche partie avec la mauvaise date est irrattrapable.)_

### 10.2 Publication : mode assisté

**Constat bloquant assumé.** Le collectif n'a pas de comptes professionnels. La publication programmée par API est donc **impossible** sur Instagram (elle exige un compte Business relié à une Page Facebook), restreinte sur TikTok, et de toute façon fermée pour les stories.

L'outil fonctionne en **mode assisté** :

1. Chaque tâche vise **un compte déclaré** (plateforme + handle), du collectif ou du groupe.
2. Un **membre se désigne** responsable de la publication, ou l'admin l'assigne. Un **suppléant** est prévu.
3. À l'heure dite, le bot lui envoie **le visuel, la légende et les hashtags**, prêts à coller.
4. Il publie depuis son téléphone, puis appuie sur **« ✅ publié »** — avec le lien du post, en option.
5. **Sans confirmation : relance à +1 h, alerte à l'admin à +3 h.** C'est ce qui garantit que la com sort réellement.

Un tableau de bord montre, par événement, l'état de chaque case : `brouillon` → `prêt` → `assigné` → `publié` / `raté`.

> **Évolution prévue, non incluse.** Le modèle `CompteSocial` et l'interface `Publisher` sont conçus pour accueillir une publication automatique le jour où des comptes professionnels existeront : un adaptateur, aucune migration de données.

### 10.3 Comptes sociaux et accès

État réel : chaque groupe a **un seul compte par plateforme**, dont le mot de passe est partagé entre ses membres. Ramas dispose de ses comptes Instagram, TikTok et YouTube ; les comptes de Bonsoir Techno sont détenus par d'autres membres du collectif.

L'app modélise donc, pour chaque compte : plateforme, handle, URL, **qui y a accès** (liste de membres), mode (partagé / personnel), et des notes.

**L'application ne stocke aucun mot de passe et ne prétend pas être un coffre-fort.** Elle répond à la seule question qui bloque réellement le jour J : _« qui peut publier sur ce compte ? »_ — et une tâche de publication ne peut être assignée qu'à quelqu'un qui y a accès. Le partage des identifiants reste dans un gestionnaire de mots de passe externe, dont l'app garde éventuellement le lien.

---

## 12. Fiches techniques

Données structurées par groupe, versionnées, exportables en **PDF** envoyable tel quel à un lieu.

**Sections :** identité (nom, style, durée du set, effectif scène) · line-up scène (membre, instrument/machine, position) · **plan de scène** (schéma téléversé ou composé depuis les positions) · **input list** (canal, source, micro/DI, insert, pied) · backline apporté / demandé · son (retours, circuits, façade) · lumière · loges et hospitalité · arrivée (déchargement, balance) · contacts techniques avec téléphone.

Depuis un événement : **« envoyer la fiche technique »** → PDF de la version en cours, attaché, contact du lieu pré-rempli.

### Absence de fiche technique : signalé, jamais bloquant

Un groupe ou un artiste peut parfaitement ne pas encore avoir de fiche technique. L'app **n'interdit alors rien** : ni la confirmation de l'événement, ni le line-up, ni la com.

Elle se contente de le **signaler** :

- mention « fiche technique manquante » sur la fiche de l'événement et dans le tableau de bord ;
- rappel unique au référent du groupe à J-14, puis plus rien — pas de harcèlement ;
- à l'envoi au lieu, les groupes sans fiche sont listés explicitement, pour que l'admin sache ce qu'il n'envoie pas.

C'est une **aide à la complétude**, pas une barrière : un concert se joue très bien sans PDF, et un outil qui refuserait de confirmer une date pour cette raison serait abandonné en une semaine.

---

## 13. Bot Telegram

Canal principal des membres : toute décision tient en deux appuis.

**Commandes** — `/start <code>` (lier son compte) · `/dispos` · `/agenda` · `/postes` · `/publier` · `/fiche` · `/collectif` (basculer, pour les membres de plusieurs collectifs)

**Messages interactifs** — sondage de dispos (✅ / ❔ / ❌ par date, 🎸 je veux jouer) · line-up retenu avec accusé de réception · poste vacant « je le prends » · tâche de publication avec visuel, texte et bouton « ✅ publié » · **alerte « on passe en live dans 15 min »** · feuille de route de la veille · relances automatiques

**Réglages par membre** — silence nocturne, fréquence des rappels, opt-out par type de notification

---

## 14. Interface web

| Écran               | Contenu                                                                                |
| ------------------- | -------------------------------------------------------------------------------------- |
| **Tableau de bord** | Ce qui bloque : sondages en attente, postes vacants, com en retard, 30 prochains jours |
| **Opportunités**    | Liste, création, matrice de dispos, arbitrage                                          |
| **Événement**       | Line-up, logistique, plan de com, visuels, fiche technique, contacts, feuille de route |
| **Calendrier**      | Mois / semaine / liste, filtres, liens iCal                                            |
| **Groupes**         | Membres, charte locale, comptes sociaux, press kit, fiches techniques                  |
| **Membres**         | Administration : création, invitations, téléphone, appartenances, droits               |
| **Lieux**           | Carnet d'adresses, contacts, historique des événements                                 |
| **Studio** | Éditeur visuel, gabarits et déclinaisons, éditeur vidéo, bibliothèque d'assets |
| **Rendus** | File des jobs vidéo, machines connectées, progression, erreurs |
| **Charte**          | Tokens du collectif, polices, logos                                                    |
| **Réglages**        | Types d'événements, jalons de com, bot, flux iCal, sauvegardes                         |
| **Instance**        | Admin d'instance uniquement : collectifs, création, santé technique                    |

Langue : **français**. Interface responsive — les admins travaillent aussi depuis un téléphone.

---

## 15. Architecture technique

**Contraintes posées :** VPS Ubuntu unique chez **OVH Cloud**, Docker, parité stricte avec le local.

### Services (Docker Compose)

| Service    | Rôle                                                                     |
| ---------- | ------------------------------------------------------------------------ |
| `web`      | Next.js (App Router, TypeScript) — interface + API                       |
| `worker`   | Jobs : rendus, relances, envois Telegram, instanciation des plans de com |
| `bot`      | Bot Telegram (grammY) — webhook en production, long polling en local     |
| `db`       | PostgreSQL                                                               |
| `storage`  | MinIO (S3-compatible) — médias, visuels, PDF                             |
| `renderer` | Chromium headless — visuels fixes et PDF. **Aucun encodage vidéo.**      |
| `proxy`    | Caddy — TLS automatique                                                  |

Un `docker-compose.yml` + surcharges `compose.local.yml` / `compose.prod.yml`. **`docker compose up` suffit à tout lancer en local**, bot compris.

### Choix techniques

- **ORM** : Drizzle — migrations SQL lisibles, versionnées dans le dépôt
- **Files d'attente et planification** : **pg-boss**, adossé à PostgreSQL. Pas de Redis : un service de moins à administrer, et les jobs planifiés survivent aux redémarrages — indispensable pour un rappel programmé à J-30.
- **Rendu des visuels fixes** : l'éditeur produit une description de mise en page ; Chromium la compose et la capture. **L'aperçu de l'éditeur et le rendu final utilisent le même moteur**, donc aucune dérive entre ce que voit l'admin et ce qui sort. Les PDF passent par la même chaîne.
- **Rendu vidéo : hors du serveur.** Le VPS n'embarque pas `ffmpeg` et n'encode jamais. Il expose une file de jobs que la **CLI Backline** réclame depuis les machines des membres (§10.3), avec accélération matérielle locale. C'est ce choix qui permet à une instance modeste d'héberger l'ensemble.
- **CLI** : paquet TypeScript distribué séparément, authentification par jeton de machine révocable, réclamation de job, téléchargement des sources, encodage, renvoi du résultat, progression en direct.
- **Authentification** : Auth.js — Telegram Login Widget pour tous, e-mail/mot de passe en secours pour les admins
- **Cloisonnement** : `collectif_id` porté par chaque table concernée, filtre appliqué au niveau de la couche d'accès aux données, couvert par des tests dédiés
- **Tests** : Vitest + **Testcontainers** (PostgreSQL et MinIO réels, jamais de simulacre) ; Playwright pour les parcours de bout en bout et la non-régression visuelle des gabarits

### Exploitation sur OVH

- VPS Ubuntu **déjà provisionné chez OVH**, privé ; accès SSH fournis ultérieurement, l'installation complète est faite depuis ce dépôt
- **Instance unique** sur `backline.betafactory.co`, certificat TLS automatique par Caddy
- **4 Go de RAM** suffisent : aucun encodage vidéo ne tourne sur le VPS ; swap configuré
- `ufw` (22/80/443 uniquement), `fail2ban`, connexion SSH par clé, mises à jour de sécurité automatiques
- Caddy pour le TLS et le renouvellement des certificats
- **Sauvegarde quotidienne** : `pg_dump` + synchronisation du bucket vers OVH Object Storage, avec rétention et **restauration testée**
- Journalisation structurée, page de santé des services, alerte Telegram en cas de service tombé
- Configuration entièrement en variables d'environnement, `.env.example` versionné
- Déploiement : `git pull && docker compose up -d --build`, **déclenché à la main**
- **Aucune intégration continue** : pas de GitHub Actions, pas de pipeline. Les tests se lancent en local (`docker compose run test`) et le push se fait manuellement. Conséquence assumée : rien n'empêche mécaniquement de pousser du rouge — la discipline remplace la barrière, et un script `make check` unique regroupe typage, lint et tests pour qu'il n'y ait qu'une commande à retenir avant de pousser.

---

## 16. Données d'amorçage

Le jeu de données installé par `docker compose up` sur une machine vierge, également utilisé par les tests de bout en bout :

**Collectif : Bonsoir Techno** — créé par **Antoine**, admin du collectif.

| Groupe | Membres |
|---|---|
| **Ramas** | Anas, Romain |
| **Dante3p** | Mathieu |

Les quatre personnes sont membres du collectif Bonsoir Techno. Ramas dispose de ses propres comptes Instagram, TikTok et YouTube ; les comptes du collectif sont détenus par Antoine (voir §11.3 — l'app recense les accès, pas les mots de passe).

S'y ajoutent, en données de démonstration : un lieu avec contact, une opportunité à trois dates candidates avec un sondage partiellement rempli, un événement confirmé complet, une résidence et un stream.

---

## 17. Ordre de construction

Le périmètre n'est pas réduit : c'est l'ordre des dépendances. Chaque étape est utilisable dès sa livraison.

1. **Socle** — Compose, base, cloisonnement multi-collectif, authentification, liaison Telegram, utilisateurs (téléphone compris), collectifs, groupes, appartenances
2. **Décision de date** — lieux, opportunités, dates candidates, sondage, matrice, conversion _(le cœur : les allers-retours disparaissent dès ce point)_
3. **Événements et logistique** — types, line-up, postes libres, volontariat, relances, feuille de route, calendrier, flux iCal
4. **Streams** — champs de diffusion, créneaux, postes techniques, alerte de mise en ligne, replay
5. **Fiches techniques** — saisie structurée, versions, export PDF _(déterministe, sans dépendance)_
6. **Charte et éditeur visuel** — tokens, canevas, blocs, alignement, catalogue de formats, déclinaisons, rendu des visuels fixes
7. **Plan de com** — timelines par type, génération des visuels, assignation, rappels, confirmation de publication
8. **Vidéo** — éditeur de plans et timeline, aperçu navigateur, file de jobs, **CLI de rendu GPU**
9. **Press kit, tableau de bord, réglages fins**

> **Dépendance dure :** l'étape 7 n'a de valeur qu'après la 6, qui exige les **vraies valeurs de charte**. C'est le seul point où le projet dépend d'une entrée extérieure au code.
>
> **Poste le plus lourd :** l'étape 6. Un éditeur visuel est un produit en soi, pas un écran de formulaire — c'est le seul endroit du projet où la complexité est réellement élevée. L'étape 8 réutilise tout son canevas : la faire avant obligerait à la refaire.

---

## 18. Critères d'acceptation

- Une date est arrêtée avec un lieu **sans aucun message manuel** : sondage ouvert → matrice → date retenue.
- Aucun poste logistique n'arrive vacant au jour J sans qu'au moins **trois relances** aient été envoyées.
- Tous les visuels d'un événement sont produits **en une action**, à tous les formats, conformes à la charte.
- Un membre **sans compétence technique** crée un gabarit complet à la souris, sans jamais voir de code.
- Le cadre d'un visuel **ne peut pas quitter le ratio** du format choisi.
- Une vidéo se décrit entièrement dans l'app et s'aperçoit dans le navigateur **sans lancer d'encodage**.
- Une composition vidéo rendue deux fois donne **deux fichiers identiques**.
- Si aucune machine GPU n'est connectée, la tâche de com reste livrable **avec son visuel fixe**, et l'admin est prévenu.
- Aucune tâche de publication ne peut être marquée publiée sans action humaine explicite ; toute tâche non confirmée **alerte un admin**.
- Une tâche de publication ne peut être assignée qu'à un membre **ayant accès au compte** visé.
- Un stream déclenche l'alerte **« on est en ligne »** à tous les membres 15 minutes avant, sans intervention.
- La fiche technique d'un groupe part en PDF vers un lieu **en moins d'une minute**.
- Un événement se confirme, se remplit et se communique **même si aucun groupe du line-up n'a de fiche technique** ; l'absence est signalée, jamais bloquante.
- Une disponibilité **ne peut être écrite que par la personne concernée**, y compris par un admin.
- Un membre voit ses événements dans son Google Calendar **sans s'être connecté à autre chose que le bot**.
- Un membre d'un collectif **ne peut accéder à aucune donnée** d'un autre collectif — vérifié par des tests.
- `docker compose up` sur une machine vierge donne une application fonctionnelle avec les données de démonstration de Bonsoir Techno et Ramas.

---

## 19. Risques

| Risque                                   | Impact                                     | Traitement                                                                                                               |
| ---------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Chartes non formalisées                  | Bloque le studio visuels                   | Tokens d'exemple livrés ; saisie des vraies valeurs sans toucher au code                                                 |
| Pas de comptes professionnels            | Pas de publication automatique             | Mode assisté (§11.2) ; adaptateur prêt pour plus tard                                                                    |
| **Mots de passe partagés entre membres** | Un départ ou une brouille bloque un compte | L'app recense **qui a accès** ; le partage reste dans un gestionnaire externe ; coffre-fort explicitement hors périmètre |
| Adoption du bot                          | Tout le flux d'entrée en dépend            | Zéro friction : pas de mot de passe, deux appuis par décision, web en doublon complet                                    |
| **Éditeur visuel : ampleur du chantier** | C'est le plus gros poste du projet | Construit en avant-dernier, sur un jeu de blocs volontairement restreint ; la bibliothèque de gabarits prêts couvre l'usage courant sans ouvrir l'éditeur |
| Aucune machine GPU connectée | Aucune vidéo ne sort | Machines connectées visibles en permanence, alerte sur job non réclamé, repli systématique sur le visuel fixe |
| Machine de rendu = accès aux médias | Fuite possible par un poste personnel | Jeton par machine, révocable ; journal des rendus ; accès limité aux médias du job |
| Fuite inter-collectifs                   | Grave, structurelle                        | Filtre au niveau de la couche de données, tests d'isolation obligatoires                                                 |
| Téléphones des membres en base           | Donnée personnelle                         | Visibilité restreinte (§3), jamais exposée publiquement ni dans les exports                                              |

---

## 20. Décisions actées

- **Multi-collectif** sur une instance ; création manuelle, aucune inscription publique. Premier collectif : Bonsoir Techno.
- Un seul concept `Groupe` ; appartenances multiples et libres ; rattachement à un collectif principal.
- Compte utilisateur unique, transverse aux collectifs ; **téléphone** au dossier.
- Application maîtresse de l'agenda ; push par abonnement iCal, jamais de synchro bidirectionnelle.
- Telegram comme canal de notification ; WhatsApp derrière une interface, non implémenté.
- Types d'événements : soirée DJ, concert, résidence, **stream live** — paramétrables, chacun avec sa timeline.
- Charte par collectif + surcharge par groupe.
- **Éditeur visuel sans code**, ratios verrouillés par format, catalogue de formats éditable ; gabarit conçu sur un format maître puis décliné.
- **Aucune IA générative** — les médias sont téléversés.
- **Vidéo décrite dans Backline, rendue par une CLI sur des machines GPU locales.** Le VPS n'encode jamais.
- **Un seul bot Telegram** pour toute l'instance ; `/collectif` bascule le contexte.
- Publication en mode assisté avec confirmation humaine ; comptes sociaux recensés **sans mots de passe**.
- Rôles logistiques en texte libre, sans catalogue.
- Créneaux horaires du line-up **optionnels**, avec un réglage distinct pour leur publication dans les visuels.
- Résidence = **un seul événement** sur une plage, avec présence déclarée jour par jour.
- Disponibilités **visibles par tous les membres** du collectif, **modifiables par leur seul auteur**, admins compris.
- Résidence : plage fixe, **présence déclarative simple** (je viens / je ne viens pas / je ne sais pas), jours précis facultatifs et jamais réclamés.
- Fiche technique manquante : **signalée, jamais bloquante**.
- Produit nommé **Backline**, marque compacte **BCKLN**.
- **Une seule instance partagée** sur `backline.betafactory.co`, VPS OVH privé — jamais un déploiement par collectif.
- **Aucun profil réservé** : le rôle d'admin est un droit attribuable à n'importe quel utilisateur, et retirable.
- Dépôt Git hébergé sur **GitHub, en privé** ; push manuel, **aucune CI**, déploiement déclenché à la main.
- VPS Ubuntu OVH, Docker Compose, parité locale/production, Testcontainers.
- Hors périmètre : argent, inventaire, setlists, billetterie, IA générative, coffre-fort de mots de passe, SaaS public.
