import React from "react";
import { Link, Navigate, Route, Routes } from "react-router-dom";
import { SessionProvider, useSession } from "./lib/session";
import { Shell } from "./components/Shell";
import { Button, Card, Loading } from "./components/ui";
import { Login } from "./screens/Login";
import { Invitation } from "./screens/Invitation";
import { Dashboard } from "./screens/Dashboard";
import { Opportunities } from "./screens/Opportunities";
import { OpportunityDetail } from "./screens/OpportunityDetail";
import { Events } from "./screens/Events";
import { EventDetail } from "./screens/EventDetail";
import { Calendar } from "./screens/Calendar";
import { Groups } from "./screens/Groups";
import { GroupDetail } from "./screens/GroupDetail";
import { Members } from "./screens/Members";
import { Venues } from "./screens/Venues";
import { Studio } from "./screens/Studio";
import { TemplateEditor } from "./studio/TemplateEditor";
import { VideoEditor } from "./studio/VideoEditor";
import { Renders } from "./screens/Renders";
import { Brand } from "./screens/Brand";
import { Settings } from "./screens/Settings";
import { Instance } from "./screens/Instance";
import { Notifications } from "./screens/Notifications";

const Protected: React.FC = () => {
  const { me, loading, collectiveId } = useSession();

  if (loading) return <Loading />;
  if (!me) return <Navigate to="/connexion" replace />;
  // Belonging to no collective has two very different meanings, and the same
  // grey sentence served both: the founder's first minute in the product, and
  // someone waiting to be added. Only one of them has something to do.
  if (!collectiveId) {
    return (
      <div className="mx-auto max-w-md px-6 py-16">
        <p className="mb-6 font-display text-lg font-semibold tracking-[0.25em]">
          BCKLN<span className="text-accent">_</span>
        </p>
        {me.is_instance_admin ? (
          <Card title="Premier collectif">
            <p className="text-sm text-ink-soft">
              L'instance tourne, ton compte est administrateur — il ne manque qu'un collectif. Il
              naîtra avec ses types d'événements, son catalogue de formats et une charte d'exemple.
            </p>
            <Link to="/instance" className="mt-4 inline-block">
              <Button variant="primary" icon="plus">
                Créer un collectif
              </Button>
            </Link>
          </Card>
        ) : (
          <Card title="En attente">
            <p className="text-sm text-ink-soft">
              Ton compte existe, mais il n'est rattaché à aucun collectif. Un administrateur doit
              t'y ajouter — d'ici là, il n'y a rien à voir ici.
            </p>
          </Card>
        )}
      </div>
    );
  }

  return (
    <Shell>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/opportunites" element={<Opportunities />} />
        <Route path="/opportunites/:id" element={<OpportunityDetail />} />
        <Route path="/evenements" element={<Events />} />
        <Route path="/evenements/:id" element={<EventDetail />} />
        <Route path="/calendrier" element={<Calendar />} />
        <Route path="/groupes" element={<Groups />} />
        <Route path="/groupes/:id" element={<GroupDetail />} />
        <Route path="/membres" element={<Members />} />
        <Route path="/lieux" element={<Venues />} />
        <Route path="/studio" element={<Studio />} />
        <Route path="/studio/gabarits/:id" element={<TemplateEditor />} />
        <Route path="/studio/videos/:id" element={<VideoEditor />} />
        <Route path="/rendus" element={<Renders />} />
        <Route path="/charte" element={<Brand />} />
        <Route path="/reglages" element={<Settings />} />
        <Route path="/notifications" element={<Notifications />} />
        <Route path="/instance" element={<Instance />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Shell>
  );
};

export const App: React.FC = () => (
  <SessionProvider>
    <Routes>
      <Route path="/connexion" element={<Login />} />
      <Route path="/invitation/:code" element={<Invitation />} />
      <Route path="/*" element={<Protected />} />
    </Routes>
  </SessionProvider>
);
