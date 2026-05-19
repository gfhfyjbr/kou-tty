# TUI Cookbook

Ready-made sequences for the most common interactive programs. Each recipe assumes a terminal was created with `ID=$(kou-tty create | jq -r .result.id)`.

## Contents

- vim (open, edit, save, quit, force-quit)
- nano
- less / man
- htop / btop
- lazygit
- fzf
- tmux (attach to an existing session)
- python REPL
- ssh password prompt
- watch / tail -f style commands

## vim

Open a file:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"vim /tmp/note.txt"},{"key":"Enter"}]'
```

Enter insert mode, type, escape:

```bash
kou-tty terminal send-keys "$ID" '[{"key":"i"},{"text":"hello from agent"},{"key":"Escape"}]'
```

Write and quit:

```bash
kou-tty terminal send-keys "$ID" '[{"text":":wq"},{"key":"Enter"}]'
```

Force-quit without saving:

```bash
kou-tty terminal send-keys "$ID" '[{"key":"Escape"},{"text":":q!"},{"key":"Enter"}]'
```

Always send `Escape` first before any `:`-command — vim may be in insert/visual mode.

## nano

```bash
kou-tty terminal send-keys "$ID" '[{"text":"nano /tmp/note.txt"},{"key":"Enter"}]'
kou-tty terminal send-keys "$ID" '[{"text":"hello"}]'
kou-tty terminal send-keys "$ID" '[{"key":"Ctrl+o"},{"key":"Enter"},{"key":"Ctrl+x"}]'
```

## less / man

Navigate one screen down, search, quit:

```bash
kou-tty terminal send-keys "$ID" '[{"key":"Space"}]'
kou-tty terminal send-keys "$ID" '[{"text":"/keyword"},{"key":"Enter"}]'
kou-tty terminal send-keys "$ID" '[{"text":"q"}]'
```

`less` only redraws when it has more content; poll `status` and check `has_new_content` between navigation steps.

## htop / btop

Both enable mouse reporting and full-screen redraw. Read the screen with `read --mode changes` to keep token usage low:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"htop"},{"key":"Enter"}]'
sleep 0.5
kou-tty terminal read "$ID" --mode full --max-lines 50
# kill the highlighted process: F9, Enter
kou-tty terminal send-keys "$ID" '[{"key":"F9"},{"key":"Enter"}]'
# quit
kou-tty terminal send-keys "$ID" '[{"key":"q"}]'
```

For btop, the quit key is the same: `q`.

## lazygit

Open lazygit and navigate panels:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"lazygit"},{"key":"Enter"}]'
kou-tty terminal send-keys "$ID" '[{"key":"Tab"}]'                # next panel
kou-tty terminal send-keys "$ID" '[{"text":" "}]'                  # stage file under cursor
kou-tty terminal send-keys "$ID" '[{"text":"c"}]'                  # commit
kou-tty terminal send-keys "$ID" '[{"text":"chore: bump"},{"key":"Enter"}]'
kou-tty terminal send-keys "$ID" '[{"text":"q"}]'                  # quit
```

## fzf

Pipe-based usage is awkward through kou-tty; instead, pre-feed input with a shell here-doc:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"ls /etc | fzf"},{"key":"Enter"}]'
sleep 0.3
kou-tty terminal send-keys "$ID" '[{"text":"hosts"},{"key":"Enter"}]'
```

## tmux

Attach to an existing session and detach cleanly:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"tmux attach -t work"},{"key":"Enter"}]'
# do work …
kou-tty terminal send-keys "$ID" '[{"key":"Ctrl+b"},{"text":"d"}]'  # detach
```

The prefix here is `Ctrl+b` (default); if the user remapped it, adjust.

## python REPL

```bash
kou-tty terminal send-keys "$ID" '[{"text":"python3"},{"key":"Enter"}]'
sleep 0.3
kou-tty terminal send-keys "$ID" '[{"text":"print(2 + 2)"},{"key":"Enter"}]'
kou-tty terminal show "$ID"
kou-tty terminal send-keys "$ID" '[{"key":"Ctrl+d"}]'  # exit
```

## ssh with a password prompt

ssh prints the password prompt only on a real TTY — perfect kou-tty use case:

```bash
kou-tty terminal send-keys "$ID" '[{"text":"ssh user@host"},{"key":"Enter"}]'
# wait until status.has_new_content == true, then check the screen for "password:"
kou-tty terminal send-keys "$ID" '[{"text":"hunter2"},{"key":"Enter"}]'
```

Never embed real secrets in scripts; pull them from a secret manager at the call site.

## watch / tail -f style commands

```bash
kou-tty terminal send-keys "$ID" '[{"text":"tail -f /var/log/system.log"},{"key":"Enter"}]'
# poll for new lines:
kou-tty terminal status "$ID"          # has_new_content true?
kou-tty terminal read "$ID" --mode changes --max-lines 20
# stop:
kou-tty terminal send-keys "$ID" '[{"key":"Ctrl+c"}]'
```
