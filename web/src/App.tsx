import React from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { SessionProvider, useSession } from "./lib/session";
import { Shell } from "./components/Shell";
import { Loading } from "./components/ui";
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
  if (!collectiveId) {
    return (
      <div className="mx-auto max-w-md p-10 text-center text-sm text-ink-soft">
        Ton compte n'est rattache a aucun collectif. Un administrateur doit t'y ajouter.
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
