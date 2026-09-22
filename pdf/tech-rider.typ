// Fiche technique — sortie deterministe, typographie propre, aucun navigateur
// dans la boucle (§15). Les donnees arrivent par `data.json`.

#let d = json("data.json")
#let group = d.at("group_name", default: "Groupe")
#let r = d.at("rider", default: (:))
#let identity = r.at("identity", default: (:))

#set document(title: "Fiche technique — " + group)
#set page(
  paper: "a4",
  margin: (x: 2cm, y: 2cm),
  footer: context [
    #set text(size: 8pt, fill: luma(120))
    #group — fiche technique v#d.at("version", default: 1)
    #h(1fr)
    #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: ("Helvetica", "DejaVu Sans", "Liberation Sans"), size: 10pt, lang: "fr")
#set par(justify: false, leading: 0.6em)

#let section(title) = {
  v(0.8em)
  block(
    width: 100%,
    inset: (y: 4pt),
    stroke: (bottom: 0.6pt + luma(160)),
    text(size: 11pt, weight: "bold", upper(title)),
  )
  v(0.3em)
}

#let field(label, value) = {
  if value != none and value != "" {
    grid(
      columns: (4.2cm, 1fr),
      gutter: 6pt,
      text(fill: luma(100), label), [#value],
    )
  }
}

// --- En-tete ---------------------------------------------------------------
#block[
  #text(size: 22pt, weight: "bold")[#group]
  #h(1fr)
  #text(size: 9pt, fill: luma(110))[Fiche technique · v#d.at("version", default: 1)]
]
#v(-0.4em)
#line(length: 100%, stroke: 1pt)

#if d.at("generated_on", default: none) != none [
  #text(size: 8pt, fill: luma(130))[Editee le #d.generated_on]
]

// --- Identite --------------------------------------------------------------
#section("Identite")
#field("Style", identity.at("style", default: none))
#field("Duree du set", if identity.at("set_duration_min", default: none) != none {
  str(identity.set_duration_min) + " min"
} else { none })
#field("Effectif scene", if identity.at("stage_headcount", default: none) != none {
  str(identity.stage_headcount)
} else { none })

// --- Line-up scene ---------------------------------------------------------
#let lineup = r.at("stage_lineup", default: ())
#if lineup.len() > 0 [
  #section("Line-up scene")
  #table(
    columns: (1fr, 1.4fr, 1fr),
    stroke: 0.4pt + luma(190),
    inset: 6pt,
    table.header(
      text(weight: "bold")[Membre],
      text(weight: "bold")[Instrument / machine],
      text(weight: "bold")[Position],
    ),
    ..lineup.map(l => (
      l.at("member", default: ""),
      l.at("instrument", default: ""),
      l.at("position", default: ""),
    )).flatten(),
  )
]

// --- Input list ------------------------------------------------------------
#let inputs = r.at("input_list", default: ())
#if inputs.len() > 0 [
  #section("Input list")
  #table(
    columns: (1.4cm, 1fr, 1fr, 1fr, 1fr),
    stroke: 0.4pt + luma(190),
    inset: 5pt,
    table.header(
      text(weight: "bold")[Ch.],
      text(weight: "bold")[Source],
      text(weight: "bold")[Micro / DI],
      text(weight: "bold")[Insert],
      text(weight: "bold")[Pied],
    ),
    ..inputs.map(i => (
      str(i.at("channel", default: "")),
      i.at("source", default: ""),
      i.at("mic", default: ""),
      i.at("insert", default: ""),
      i.at("stand", default: ""),
    )).flatten(),
  )
]

// --- Backline --------------------------------------------------------------
#let backline = r.at("backline", default: (:))
#if backline.len() > 0 [
  #section("Backline")
  #field("Apporte", backline.at("brought", default: none))
  #field("Demande", backline.at("requested", default: none))
]

// --- Son / lumiere ---------------------------------------------------------
#let sound = r.at("sound", default: (:))
#if sound.len() > 0 [
  #section("Son")
  #field("Retours", sound.at("monitors", default: none))
  #field("Circuits", sound.at("circuits", default: none))
  #field("Facade", sound.at("foh", default: none))
]

#if r.at("light", default: none) != none [
  #section("Lumiere")
  #r.light
]

#if r.at("hospitality", default: none) != none [
  #section("Loges et hospitalite")
  #r.hospitality
]

// --- Arrivee ---------------------------------------------------------------
#let arrival = r.at("arrival", default: (:))
#if arrival.len() > 0 [
  #section("Arrivee")
  #field("Dechargement", arrival.at("load_in", default: none))
  #field("Balance", arrival.at("soundcheck", default: none))
]

// --- Contacts --------------------------------------------------------------
#let contacts = r.at("contacts", default: ())
#if contacts.len() > 0 [
  #section("Contacts techniques")
  #table(
    columns: (1fr, 1fr, 1fr),
    stroke: 0.4pt + luma(190),
    inset: 6pt,
    table.header(
      text(weight: "bold")[Nom],
      text(weight: "bold")[Role],
      text(weight: "bold")[Telephone],
    ),
    ..contacts.map(c => (
      c.at("name", default: ""),
      c.at("role", default: ""),
      c.at("phone", default: ""),
    )).flatten(),
  )
]
