import { useState } from 'preact/hooks';
import { api, pickFolder } from './api';
import { Home } from './Home';
import { NewProject } from './NewProject';
import { Toaster, messageOf, notify } from './notify';
import { UpdateBanner } from './UpdateBanner';
import type { Opened } from './types';
import { Workspace } from './Workspace';

type Screen = { name: 'home' } | { name: 'new'; importGirls?: boolean } | { name: 'workspace'; opened: Opened; importGirls?: boolean };

export function App() {
  const [screen, setScreen] = useState<Screen>({ name: 'home' });

  const open = async (path: string) => {
    try {
      setScreen({ name: 'workspace', opened: await api.openProject(path) });
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const openFolder = async () => {
    const path = await pickFolder('Open a data pack folder');

    if (path) await open(path);
  };

  const home = async () => {
    await api.closeProject().catch(() => undefined);
    setScreen({ name: 'home' });
  };

  return (
    <>
      {screen.name === 'home' && <Home onNew={() => setScreen({ name: 'new' })} onNewFromGirls={() => setScreen({ name: 'new', importGirls: true })} onOpen={open} onOpenFolder={openFolder} />}
      {screen.name === 'new' && <NewProject onCancel={() => setScreen({ name: 'home' })} onCreated={opened => setScreen({ name: 'workspace', opened, importGirls: screen.importGirls })} />}
      {screen.name === 'workspace' && <Workspace key={screen.opened.root} opened={screen.opened} onHome={home} startImport={screen.importGirls} />}
      <Toaster />
      <UpdateBanner />
    </>
  );
}
