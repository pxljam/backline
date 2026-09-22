/**
 * Automatic event fields (§9.1). They are dropped onto the canvas and filled
 * in at generation time. The paths match exactly what
 * `GET /studio/preview-fields/:event_id` returns.
 */
export const EVENT_FIELDS: { path: string; label: string }[] = [
  { path: "event.title", label: "Titre de l'evenement" },
  { path: "event.date", label: "Date (12.06.2026)" },
  { path: "event.date_long", label: "Date en toutes lettres" },
  { path: "event.weekday", label: "Jour de la semaine" },
  { path: "event.time", label: "Heure de debut" },
  { path: "event.end_time", label: "Heure de fin" },
  { path: "venue.name", label: "Lieu" },
  { path: "venue.city", label: "Ville" },
  { path: "collective.name", label: "Collectif" },
  { path: "line_up_text", label: "Line-up (une ligne par groupe)" },
];

export function fieldToken(path: string): string {
  return `{{${path}}}`;
}

/** Sample values, for working on a template without an event. */
export const SAMPLE_DATA = {
  event: {
    title: "Bonsoir Techno #12",
    date: "12.06.2026",
    date_long: "12 juin 2026",
    weekday: "vendredi",
    time: "22:00",
    end_time: "04:00",
  },
  collective: { name: "Bonsoir Techno" },
  venue: { name: "La Friche", city: "Nantes" },
  line_up: [{ name: "Ramas", slot: null }],
  line_up_text: "Ramas\nInvite surprise",
};
