/* The Pleasure Horizons Pack Creator's link to Blockbench. It lets the Pack Creator open a model in a Blockbench that is already running: the Pack Creator leaves a job file in a
   folder, this plugin picks it up and runs it, and answers in a result file. It also writes a sign of life, so the Pack Creator knows Blockbench can be reached this way. */
(function () {
  const fs = require('fs');
  const path = require('path');
  const dir = path.join(process.env.LOCALAPPDATA || '', 'PleasureHorizonsPackCreator', 'blockbench', 'jobs');
  let timer = null;

  BBPlugin.register('pc_bridge', {
    title: 'Pack Creator link',
    author: 'Pleasure Horizons Pack Creator',
    description: 'Lets the Pleasure Horizons Pack Creator open models and animations in this Blockbench while it is already open.',
    icon: 'link',
    version: '1.0.0',
    variant: 'desktop',
    onload() {
      try { fs.mkdirSync(dir, { recursive: true }); } catch (e) { /* tried again on the next beat */ }

      let busy = false;

      timer = setInterval(async () => {
        try { fs.writeFileSync(path.join(dir, 'alive'), String(Date.now())); } catch (e) { /* the folder may be gone for a moment */ }

        if (busy) return;

        let names = [];

        try { names = fs.readdirSync(dir).filter(n => n.endsWith('.job')); } catch (e) { return; }

        for (const name of names) {
          busy = true;

          let result;

          try {
            const file = path.join(dir, name);
            const job = JSON.parse(fs.readFileSync(file, 'utf8'));

            fs.unlinkSync(file);
            result = await (0, eval)(job.script)(job.data);
          } catch (e) {
            result = 'Blockbench reported: ' + ((e && e.message) || e);
          }

          try { fs.writeFileSync(path.join(dir, name.replace(/\.job$/, '.result')), String(result)); } catch (e) { /* nothing is waiting */ }

          busy = false;
        }
      }, 500);
    },
    onunload() {
      if (timer) clearInterval(timer);

      try { fs.unlinkSync(path.join(dir, 'alive')); } catch (e) { /* already gone */ }
    }
  });
})();
