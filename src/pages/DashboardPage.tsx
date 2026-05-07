/**
 * src/pages/DashboardPage.tsx
 *
 * Dashboard landing page. Provides quick actions (create environment) and
 * showcases featured stacks via the FeaturedCarousel. Uses CreateEnvModal to
 * start the creation flow and delegates list refresh to EnvironmentsContext.
 */

import { useState } from "react";
import { useBusy } from "../context/BusyContext";
import { useEnvironments } from "../context/EnvironmentsContext";
import CreateEnvModal, { TemplateId } from "../components/CreateEnvModal";
import FeaturedCarousel from "../components/FeaturedCarousel";

type CreateDefaults = { template: TemplateId; name: string; home: string } | null;

export default function DashboardPage() {
  const [showCreate, setShowCreate] = useState<boolean>(false);
  const [defaults, setDefaults] = useState<CreateDefaults>(null);
  const { startBusy, endBusy } = useBusy();
  const { refresh } = useEnvironments();

  const fetchList = async () => {
    startBusy();
    try {
      await refresh();
    } finally {
      endBusy();
    }
  };

  const openCreateWith = (t: TemplateId) => {
    const nameMap: Record<TemplateId, string> = {
      "react-ts": "ReactTS_Project",
      python: "Python_Project",
      cpp: "CPP_Project",
      rust: "Rust_Project",
      java: "Java_Project",
      csharp: "CSharp_Project",
    };
    const suggestedName = nameMap[t];
    const home = `$HOME/EnvStation/Projects/${suggestedName}`; // will be expanded by the backend
    setDefaults({ template: t, name: suggestedName, home });
    setShowCreate(true);
  };

  return (
    <section className="dashboard-split">
      <div className="actions actions-top" data-tauri-drag-region="none">
        <button className="action-banner-btn" onClick={() => { setDefaults(null); setShowCreate(true); }} data-tauri-drag-region="none">
          New Fedora environment
        </button>

      </div>

      <div className="carousel-wrap">
        <FeaturedCarousel onSelect={(key) => openCreateWith(key as TemplateId)} />
      </div>

      {showCreate && (
        <CreateEnvModal
          defaultTemplate={defaults?.template}
          defaultName={defaults?.name}
          defaultHomeMount={defaults?.home}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false);
            fetchList();
          }}
        />
      )}
    </section>
  );
}
