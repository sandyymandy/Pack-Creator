import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { messageOf, notify } from './notify';
import { DocsDialog } from './DocsDialog';
import { Tour } from './Tour';
import { ALL_TOURS, markTours, tourSeen } from './tourState';
import { homeSteps } from './tourSteps';
import type { RecentView } from './types';

function ago(seconds: number): string {
  const diff = Math.max(0, Date.now() / 1000 - seconds);
  const unit = (value: number, name: string) => `${value} ${name}${value === 1 ? '' : 's'} ago`;

  if (diff < 90) return 'just now';
  if (diff < 3600) return unit(Math.round(diff / 60), 'minute');
  if (diff < 86400) return unit(Math.round(diff / 3600), 'hour');
  if (diff < 86400 * 30) return unit(Math.round(diff / 86400), 'day');

  return new Date(seconds * 1000).toLocaleDateString();
}

export function Home({ onNew, onNewFromGirls, onOpen, onOpenFolder }: { onNew: () => void; onNewFromGirls: () => void; onOpen: (path: string) => void; onOpenFolder: () => void }) {
  const [recent, setRecent] = useState<RecentView[] | null>(null);
  const [docs, setDocs] = useState<string | null>(null);
  const [touring, setTouring] = useState(false);

  const load = () => api.listRecent().then(setRecent).catch(error => notify(messageOf(error)));

  useEffect(() => { void load(); }, []);

  /* The tour opens by itself the first time, and after that only when asked for. */
  useEffect(() => {
    if (tourSeen('home')) return;

    const timer = window.setTimeout(() => setTouring(true), 500);

    return () => window.clearTimeout(timer);
  }, []);

  const endTour = (skipped: boolean) => {
    markTours(...(skipped ? ALL_TOURS : (['home'] as const)));
    setTouring(false);
  };

  const remove = async (event: Event, path: string) => {
    event.stopPropagation();
    await api.forgetRecent(path).catch(error => notify(messageOf(error)));
    await load();
  };

  return (
    <div class="home">
      <div class="home-inner">
        <div class="brand">
          <img src="/icon.png" alt="" />
          <h1>Pack Creator</h1>
        </div>
        <p class="tagline">Make girls, scenes, traits and dialogue for Pleasure Horizons, with the game's own rules checking as you go.</p>

        <div class="home-actions">
          <button class="btn primary" data-tour="home-new" onClick={onNew}>New project</button>
          <button class="btn" data-tour="home-import" title="Make a project and bring old girls in from their resource packs straight away" onClick={onNewFromGirls}>New project from old girls</button>
          <button class="btn" data-tour="home-open" onClick={onOpenFolder}>Open a folder</button>
          <button class="btn" data-tour="home-docs" onClick={() => setDocs('')}>Documentation</button>
          <button class="btn" onClick={() => setTouring(true)}>Take the tour</button>
        </div>

        <h2 class="section-title" data-tour="home-projects">Your projects</h2>

        {recent === null ? null : recent.length === 0 ? (
          <div class="empty">No projects yet. A project is a data pack: make one with <b>New project</b>, or open a pack you already have.</div>
        ) : (
          <div class="cards">
            {recent.map(project => (
              <button key={project.path} class={'card' + (project.exists ? '' : ' missing')} onClick={() => (project.exists ? onOpen(project.path) : notify('That folder is gone. Remove it from the list, or move it back.'))}>
                <span class="title">{project.name}</span>
                <span class="path" title={project.path}>{project.path}</span>
                <span class="when">{project.exists ? 'Opened ' + ago(project.last_opened) : 'Folder not found'}</span>
                <span class="btn ghost small remove" title="Remove from the list (the folder is kept)" onClick={event => remove(event, project.path)}>Remove</span>
              </button>
            ))}
          </div>
        )}
      </div>
      {docs !== null && <DocsDialog start={docs || undefined} onClose={() => setDocs(null)} onTour={() => { setDocs(null); setTouring(true); }} />}
      {touring && <Tour steps={homeSteps(page => { endTour(false); setDocs(page); })} onDone={() => endTour(false)} onSkip={() => endTour(true)} />}
    </div>
  );
}
