import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Button, Empty, Input } from "./ui";
import { computeSteps } from "./FirstSteps";
import type { Dashboard } from "../lib/types";

/**
 * The first component tests here, kept to what can break silently.
 *
 * `pnpm run check` cannot see a broken palette — a wrong class string typechecks
 * and renders. So these assert behaviour and the two things a future class-string
 * edit could quietly remove: the focus ring, and the empty-state action.
 */

describe("Empty", () => {
  it("stays a plain sentence when nothing can be done about it", () => {
    render(<Empty>Rien ne bloque.</Empty>);
    expect(screen.getByText("Rien ne bloque.")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("carries an action and an explanation when there is one", () => {
    render(
      <Empty icon="group" hint="Un groupe, c'est un projet qui joue." action={<Button>Créer</Button>}>
        Aucun groupe.
      </Empty>,
    );
    expect(screen.getByText("Aucun groupe.")).toBeInTheDocument();
    expect(screen.getByText("Un groupe, c'est un projet qui joue.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Créer" })).toBeInTheDocument();
  });
});

describe("focus", () => {
  it("keeps a visible focus ring on buttons and inputs", () => {
    // Nothing in the application showed keyboard focus before the redesign;
    // this is the guard against losing it again to a class-string edit.
    const { container } = render(
      <>
        <Button>Créer</Button>
        <Input aria-label="Nom" />
      </>,
    );
    for (const el of container.querySelectorAll("button, input")) {
      expect(el.className).toMatch(/focus-visible:(ring|outline)/);
    }
  });
});

const base: Dashboard = {
  setup: { members: 1, groups: 0, venues: 0, opportunities: 0, templates: 0, brand_has_logo: false },
  pending_polls: [],
  vacant_slots: [],
  late_tasks: [],
  upcoming: [],
  missing_tech_riders: [],
  render_machines_online: 0,
  unread_notifications: 0,
  is_admin: true,
};

describe("computeSteps", () => {
  it("lists everything a brand new collective still has to do", () => {
    const steps = computeSteps(base, true);
    expect(steps).toHaveLength(7);
    expect(steps.every((s) => !s.done)).toBe(true);
  });

  it("ticks a step off as soon as the data says so — nothing is stored", () => {
    const steps = computeSteps(
      { ...base, setup: { ...base.setup!, groups: 2, members: 4 }, render_machines_online: 1 },
      true,
    );
    const done = steps.filter((s) => s.done).map((s) => s.key);
    expect(done).toEqual(["members", "groups", "render"]);
  });

  it("gives a member their own short list, never an admin's chores", () => {
    const steps = computeSteps(base, false);
    expect(steps.map((s) => s.key)).not.toContain("members");
    expect(steps.length).toBeLessThan(4);
  });

  it("says nothing at all when the payload predates the setup block", () => {
    const { setup: _, ...older } = base;
    expect(computeSteps(older as Dashboard, true)).toEqual([]);
  });
});
