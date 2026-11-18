/* The Pleasure Horizons Pack Creator's link to Blockbench. It lets the Pack Creator open a model in a Blockbench that is already running: the Pack Creator leaves a job file in a
   folder, this plugin picks it up and runs it, and answers in a result file. It also writes a sign of life, so the Pack Creator knows Blockbench can be reached this way.

   Blockbench gives a plugin no `process` and asks the person before it hands over `fs`, so the folder is written in by the Pack Creator when it puts this file down, the plugin
   is registered first (so Blockbench shows what it is even if a permission is refused), and the file system is asked for once, for that one folder. */
(function () {
  const DIR = '__JOBS_DIR__';
  const VERSION = '1.0.0';
  let timer = null;

  BBPlugin.register('pc_bridge', {
    title: 'Pack Creator Link',
    author: 'Pleasure Horizons Pack Creator',
    description: 'Lets the Pleasure Horizons Pack Creator open models and animations in this Blockbench while it is already open.',
    about: 'The Pleasure Horizons Pack Creator uses this plugin to open a girl\'s model, texture and animations in the Blockbench you already have open, instead of starting another one.\n\n'
      + 'It watches one folder for job files from the Pack Creator (`' + DIR + '`) and writes a small file there every half second to show it is running. It does nothing else, and it needs permission to use files in that folder only.',
    icon: 'link',
    version: VERSION,
    variant: 'desktop',
    onload() {
      let fs = null;

      try {
        fs = require('fs', { scope: DIR, message: 'Lets the Pack Creator hand models to this Blockbench through one folder.' });
      } catch (e) { /* refused or not available */ }

      if (!fs) {
        try { Blockbench.showQuickMessage('Pack Creator Link: without access to its folder, models cannot be opened in this Blockbench.', 4000); } catch (e) { /* no message */ }
        return;
      }

      try { fs.mkdirSync(DIR, { recursive: true }); } catch (e) { /* tried again on the next beat */ }

      let busy = false;

      timer = setInterval(async () => {
        try { fs.writeFileSync(DIR + '/alive', String(Date.now())); } catch (e) { /* the folder may be gone for a moment */ }

        if (busy) return;

        let names = [];

        try { names = fs.readdirSync(DIR).filter(n => n.endsWith('.job')); } catch (e) { return; }

        for (const name of names) {
          busy = true;

          let result;

          try {
            const file = DIR + '/' + name;
            const job = JSON.parse(fs.readFileSync(file, 'utf8'));

            fs.unlinkSync(file);
            result = await (0, eval)(job.script)(job.data);
          } catch (e) {
            result = 'Blockbench reported: ' + ((e && e.message) || e);
          }

          try { fs.writeFileSync(DIR + '/' + name.replace(/\.job$/, '.result'), String(result)); } catch (e) { /* nothing is waiting */ }

          busy = false;
        }
      }, 500);
    },
    onunload() {
      if (timer) clearInterval(timer);
      timer = null;
    }
  });
})();
