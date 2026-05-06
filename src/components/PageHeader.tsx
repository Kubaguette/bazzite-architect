import React from "react";

interface Props {
  title: string;
  actions?: React.ReactNode;
}

export default function PageHeader({ title, actions }: Props) {
  return (
    <header className="page-header" style={{ alignItems: "center", gap: 12 }}>
      <h1 style={{ margin: 0 }}>{title}</h1>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>{actions}</div>
    </header>
  );
}
