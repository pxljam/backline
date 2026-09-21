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
| Studio visuels               | Gabarits déterministes éditables, rendu image et vidéo multi-formats          |
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
- **Publication automatique via API** des réseaux sociaux → §10.2
- **Génération d'images ou de vidéos par IA** : les médias sont **fournis et téléversés**. La chaîne de génération sera travaillée séparément, hors de cet outil ; l'app se contente d'accueillir les fichiers produits.
- **Stockage des mots de passe** des comptes sociaux → §10.3
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
      ├── Gabarit[]                   (collectif ou groupe, formats multiples)
      └── Asset[]                     (médias téléversés, visuels rendus, PDF)

Utilisateur                           (transverse : nom, téléphone, e-mail, Telegram)
```

### Règles de domaine

- **Cloisonnement strict par collectif** : toute requête est filtrée par le ou les collectifs de l'utilisateur. C'est une règle d'accès vérifiée au niveau des données, pas seulement de l'interface.
- Un **utilisateur appartient à plusieurs collectifs** et à **plusieurs groupes**, sans contrainte de combinaison.
- Un **groupe est rattaché à un collectif principal** (Ramas → Bonsoir Techno) — ce qui détermine l'héritage de charte. Il peut néanmoins **participer à des événements d'un autre collectif** en tant qu'invité.
- Le **porteur** d'un événement est soit le collectif, soit un groupe. Même objet, champ `porteur`.
- Un **groupe d'une personne** est un groupe comme un autre.
- Un **rôle logistique est une chaîne libre**, saisie à l'événement. Pas de catalogue ; autocomplétion sur les libellés déjà utilisés dans le collectif.
- Une **fiche technique est versionnée** ; l'événement pointe vers la version réellement envoyée au lieu.

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
| **Soirée DJ électro** | Line-up à créneaux (b2b, warm-up, closing), lieu, jauge                                                                             | §10.1           |
| **Concert**           | Line-up scène, balances, fiche technique obligatoire                                                                                | §10.1           |
| **Résidence**         | **Un seul événement** couvrant toute la plage de dates, présence partielle des membres, objectif de travail, restitution éventuelle | §10.1 (allégée) |
| **Stream live**       | Voir ci-dessous                                                                                                                     | §10.1 (courte)  |

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

### 9.1 Gabarits

Un **gabarit déterministe** = une mise en page (HTML/CSS) + des **emplacements** typés :

- `texte` : nom d'artiste, date, lieu, mentions
- `média` : image ou vidéo de fond, **téléversée** ou tirée du press kit
- `logo` : résolu automatiquement depuis la charte du porteur
- `champ automatique` : date, heure, ville, line-up, créneaux — pré-remplis depuis l'événement

Le respect de la charte est **structurel** : un gabarit ne référence que des tokens, jamais des valeurs en dur.

### 9.2 Éditeur de gabarits

Écran dédié :

- Édition de la mise en page avec **aperçu en direct sur les données réelles** d'un événement
- Déclaration des emplacements (nom, type, obligatoire, longueur max)
- Déclinaison **multi-formats** depuis un même gabarit : `1:1`, `4:5`, `9:16`, `16:9`
- Gabarits partagés au niveau du collectif, ou propres à un groupe
- Versionnement : modifier un gabarit ne casse aucun visuel déjà produit

### 9.3 Génération

Depuis un événement : **« générer les visuels »** → produit en une passe tous les formats du plan de com, stockés dans la bibliothèque d'assets et attachés aux tâches de publication.

### 9.4 Médias

**Téléversement uniquement** : photos, captations, visuels produits ailleurs. Bibliothèque par collectif et par groupe, avec étiquettes et réutilisation.

La **génération d'images par IA est hors périmètre** : elle sera travaillée avec des outils dédiés, en dehors de l'app, et les fichiers produits seront simplement téléversés. Aucun fournisseur d'IA n'est intégré, aucune clé à gérer, aucun coût variable.

---

## 10. Plan de com et publication

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

## 11. Fiches techniques

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

## 12. Bot Telegram

Canal principal des membres : toute décision tient en deux appuis.

**Commandes** — `/start <code>` (lier son compte) · `/dispos` · `/agenda` · `/postes` · `/publier` · `/fiche` · `/collectif` (basculer, pour les membres de plusieurs collectifs)

**Messages interactifs** — sondage de dispos (✅ / ❔ / ❌ par date, 🎸 je veux jouer) · line-up retenu avec accusé de réception · poste vacant « je le prends » · tâche de publication avec visuel, texte et bouton « ✅ publié » · **alerte « on passe en live dans 15 min »** · feuille de route de la veille · relances automatiques

**Réglages par membre** — silence nocturne, fréquence des rappels, opt-out par type de notification

---

## 13. Interface web

| Écran               | Contenu                                                                                |
| ------------------- | -------------------------------------------------------------------------------------- |
| **Tableau de bord** | Ce qui bloque : sondages en attente, postes vacants, com en retard, 30 prochains jours |
| **Opportunités**    | Liste, création, matrice de dispos, arbitrage                                          |
| **Événement**       | Line-up, logistique, plan de com, visuels, fiche technique, contacts, feuille de route |
| **Calendrier**      | Mois / semaine / liste, filtres, liens iCal                                            |
| **Groupes**         | Membres, charte locale, comptes sociaux, press kit, fiches techniques                  |
| **Membres**         | Administration : création, invitations, téléphone, appartenances, droits               |
| **Lieux**           | Carnet d'adresses, contacts, historique des événements                                 |
| **Studio**          | Gabarits, éditeur, bibliothèque d'assets                                               |
| **Charte**          | Tokens du collectif, polices, logos                                                    |
| **Réglages**        | Types d'événements, jalons de com, bot, flux iCal, sauvegardes                         |
| **Instance**        | Admin d'instance uniquement : collectifs, création, santé technique                    |

Langue : **français**. Interface responsive — les admins travaillent aussi depuis un téléphone.

---

## 14. Architecture technique

**Contraintes posées :** VPS Ubuntu unique chez **OVH Cloud**, Docker, parité stricte avec le local.

### Services (Docker Compose)

| Service    | Rôle                                                                     |
| ---------- | ------------------------------------------------------------------------ |
| `web`      | Next.js (App Router, TypeScript) — interface + API                       |
| `worker`   | Jobs : rendus, relances, envois Telegram, instanciation des plans de com |
| `bot`      | Bot Telegram (grammY) — webhook en production, long polling en local     |
| `db`       | PostgreSQL                                                               |
| `storage`  | MinIO (S3-compatible) — médias, visuels, PDF                             |
| `renderer` | Chromium headless + ffmpeg — visuels et PDF                              |
| `proxy`    | Caddy — TLS automatique                                                  |

Un `docker-compose.yml` + surcharges `compose.local.yml` / `compose.prod.yml`. **`docker compose up` suffit à tout lancer en local**, bot compris.

### Choix techniques

- **ORM** : Drizzle — migrations SQL lisibles, versionnées dans le dépôt
- **Files d'attente et planification** : **pg-boss**, adossé à PostgreSQL. Pas de Redis : un service de moins à administrer, et les jobs planifiés survivent aux redémarrages — indispensable pour un rappel programmé à J-30.
- **Rendu** : les gabarits sont du HTML/CSS, Chromium les capture. **L'aperçu de l'éditeur et le rendu final utilisent le même moteur**, donc aucune dérive entre ce que voit l'admin et ce qui sort. `ffmpeg` compose les formats vidéo. Les PDF passent par la même chaîne.
- **Authentification** : Auth.js — Telegram Login Widget pour tous, e-mail/mot de passe en secours pour les admins
- **Cloisonnement** : `collectif_id` porté par chaque table concernée, filtre appliqué au niveau de la couche d'accès aux données, couvert par des tests dédiés
- **Tests** : Vitest + **Testcontainers** (PostgreSQL et MinIO réels, jamais de simulacre) ; Playwright pour les parcours de bout en bout et la non-régression visuelle des gabarits

### Exploitation sur OVH

- VPS Ubuntu **déjà provisionné chez OVH**, privé ; accès SSH fournis ultérieurement, l'installation complète est faite depuis ce dépôt
- **Instance unique** sur `backline.betafactory.co`, certificat TLS automatique par Caddy
- **4 Go de RAM minimum, 8 Go recommandés** si les rendus vidéo sont fréquents ; swap configuré
- `ufw` (22/80/443 uniquement), `fail2ban`, connexion SSH par clé, mises à jour de sécurité automatiques
- Caddy pour le TLS et le renouvellement des certificats
- **Sauvegarde quotidienne** : `pg_dump` + synchronisation du bucket vers OVH Object Storage, avec rétention et **restauration testée**
- Journalisation structurée, page de santé des services, alerte Telegram en cas de service tombé
- Configuration entièrement en variables d'environnement, `.env.example` versionné
- Déploiement : `git pull && docker compose up -d --build`

---

## 15. Données d'amorçage

Le jeu de données installé par `docker compose up` sur une machine vierge, également utilisé par les tests de bout en bout :

**Collectif : Bonsoir Techno** — créé par **Antoine**, admin du collectif.

| Groupe | Membres |
|---|---|
| **Ramas** | Anas, Romain |
| **Dante3p** | Mathieu |

Les quatre personnes sont membres du collectif Bonsoir Techno. Ramas dispose de ses propres comptes Instagram, TikTok et YouTube ; les comptes du collectif sont détenus par Antoine (voir §10.3 — l'app recense les accès, pas les mots de passe).

S'y ajoutent, en données de démonstration : un lieu avec contact, une opportunité à trois dates candidates avec un sondage partiellement rempli, un événement confirmé complet, une résidence et un stream.

---

## 16. Ordre de construction

Le périmètre n'est pas réduit : c'est l'ordre des dépendances. Chaque étape est utilisable dès sa livraison.

1. **Socle** — Compose, base, cloisonnement multi-collectif, authentification, liaison Telegram, utilisateurs (téléphone compris), collectifs, groupes, appartenances
2. **Décision de date** — lieux, opportunités, dates candidates, sondage, matrice, conversion _(le cœur : les allers-retours disparaissent dès ce point)_
3. **Événements et logistique** — types, line-up, postes libres, volontariat, relances, feuille de route, calendrier, flux iCal
4. **Streams** — champs de diffusion, créneaux, postes techniques, alerte de mise en ligne, replay
5. **Fiches techniques** — saisie structurée, versions, export PDF _(déterministe, sans dépendance)_
6. **Charte et gabarits** — tokens, éditeur, rendu multi-formats
7. **Plan de com** — timelines par type, génération des visuels, assignation, rappels, confirmation de publication
8. **Press kit, tableau de bord, réglages fins**

> **Dépendance dure :** l'étape 7 n'a de valeur qu'après la 6, qui exige les **vraies valeurs de charte**. C'est le seul point où le projet dépend d'une entrée extérieure au code.

---

## 17. Critères d'acceptation

- Une date est arrêtée avec un lieu **sans aucun message manuel** : sondage ouvert → matrice → date retenue.
- Aucun poste logistique n'arrive vacant au jour J sans qu'au moins **trois relances** aient été envoyées.
- Tous les visuels d'un événement sont produits **en une action**, à tous les formats, conformes à la charte.
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

## 18. Risques

| Risque                                   | Impact                                     | Traitement                                                                                                               |
| ---------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Chartes non formalisées                  | Bloque le studio visuels                   | Tokens d'exemple livrés ; saisie des vraies valeurs sans toucher au code                                                 |
| Pas de comptes professionnels            | Pas de publication automatique             | Mode assisté (§10.2) ; adaptateur prêt pour plus tard                                                                    |
| **Mots de passe partagés entre membres** | Un départ ou une brouille bloque un compte | L'app recense **qui a accès** ; le partage reste dans un gestionnaire externe ; coffre-fort explicitement hors périmètre |
| Adoption du bot                          | Tout le flux d'entrée en dépend            | Zéro friction : pas de mot de passe, deux appuis par décision, web en doublon complet                                    |
| Rendu vidéo sur un VPS modeste           | Exports lents                              | Rendus en file d'attente, jamais en synchrone ; dimensionnement RAM anticipé                                             |
| Fuite inter-collectifs                   | Grave, structurelle                        | Filtre au niveau de la couche de données, tests d'isolation obligatoires                                                 |
| Téléphones des membres en base           | Donnée personnelle                         | Visibilité restreinte (§3), jamais exposée publiquement ni dans les exports                                              |

---

## 19. Décisions actées

- **Multi-collectif** sur une instance ; création manuelle, aucune inscription publique. Premier collectif : Bonsoir Techno.
- Un seul concept `Groupe` ; appartenances multiples et libres ; rattachement à un collectif principal.
- Compte utilisateur unique, transverse aux collectifs ; **téléphone** au dossier.
- Application maîtresse de l'agenda ; push par abonnement iCal, jamais de synchro bidirectionnelle.
- Telegram comme canal de notification ; WhatsApp derrière une interface, non implémenté.
- Types d'événements : soirée DJ, concert, résidence, **stream live** — paramétrables, chacun avec sa timeline.
- Charte par collectif + surcharge par groupe.
- Gabarits déterministes éditables ; **aucune IA générative** — les médias sont téléversés.
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
- Dépôt Git hébergé sur **GitHub, en privé**.
- VPS Ubuntu OVH, Docker Compose, parité locale/production, Testcontainers.
- Hors périmètre : argent, inventaire, setlists, billetterie, IA générative, coffre-fort de mots de passe, SaaS public.
