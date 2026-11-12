This directory is a LÖVE 11.5 web game. Serve it from an HTTP(S) server;
opening index.html with file:// will not work.

Required response headers:
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Content-Security-Policy: script-src 'self' 'unsafe-eval'
Serve .wasm files as application/wasm.

For local testing with miniserve (from this directory):
miniserve -i 127.0.0.1 --index index.html \
  --header 'Cross-Origin-Opener-Policy:same-origin' \
  --header 'Cross-Origin-Embedder-Policy:require-corp' \
  --header "Content-Security-Policy:script-src 'self' 'unsafe-eval'" .

The _headers file configures static Netlify and Cloudflare Pages deployments
when this directory is published as the site root.
Apache uses .htaccess when AllowOverride and mod_headers are enabled. Other
hosts need equivalent headers. Check the response headers in your browser's
Network panel. For itch.io HTML uploads, enable SharedArrayBuffer support in
the game's Embed Options. Browser audio and fullscreen may require a user
gesture. See https://github.com/2dengine/love.js for player limitations and
licenses.
