# Cloudflare Pages Deployment - Quick Start

Get your transmitwave app deployed to Cloudflare Pages in 5 minutes.

## Option A: Manual GitHub Actions Deployment (On-Demand)

### Step 1: Create Cloudflare API Token
1. Go to https://dash.cloudflare.com/profile/api-tokens
2. Click "Create Token"
3. Use template "Edit Cloudflare Pages" (or create custom with Pages Edit permission)
4. Copy the token

### Step 2: Add GitHub Secrets
1. Go to your GitHub repo → Settings → Secrets and variables → Actions
2. Click "New repository secret"
3. Add these secrets:
   - Name: `CLOUDFLARE_API_TOKEN` → Value: (paste token from Step 1)
   - Name: `CLOUDFLARE_ACCOUNT_ID` → Value: (from Cloudflare Dashboard top-right)

### Step 3: Trigger Deployment On-Demand
1. Go to GitHub repo → Actions tab
2. Click "Deploy to Cloudflare Pages" workflow
3. Click "Run workflow" button
4. Wait for green checkmark (5-10 minutes)
5. Your site will be live at `transmitwave.[your-domain].pages.dev`

**Note:** Workflow only runs when you manually trigger it. No automatic deployments on push.

## Option B: Manual Deployment with Wrangler CLI

### Step 1: Local Build
```bash
# From project root
bash setup-cloudflare.sh
```

Or manually:
```bash
# Build WASM
cd wasm && bash build.sh && cd ..

# Build web app
cd web && npm install && npm run build && cd ..
```

### Step 2: Install Wrangler
```bash
npm install -g wrangler
```

### Step 3: Authenticate
```bash
wrangler login
# This opens browser to authenticate with Cloudflare
```

### Step 4: Deploy
```bash
wrangler pages deploy web/dist --project-name=transmitwave
```

Your site is now live!

## Option C: Connect GitHub to Cloudflare Pages Dashboard

### Step 1: Create Project in Cloudflare
1. Go to https://dash.cloudflare.com/
2. Select account → Pages → Create application
3. Select "Connect to Git"
4. Choose GitHub and authorize
5. Select your `transmitwave` repository

### Step 2: Configure Build Settings
- **Production branch:** `main`
- **Build command:** `npm run build:all`
- **Build output directory:** `web/dist`
- **Root directory:** `.` (or leave blank)

### Step 3: Add Environment Variables (Optional)
Add any secrets your app needs (leave empty if using defaults)

### Step 4: Deploy
Click "Save and Deploy"

Cloudflare will automatically build and deploy every time you push to `main`.

## Verify Deployment

```bash
# Check deployment status
wrangler pages list --project-name=transmitwave

# Open your site
open https://transmitwave.[your-domain].pages.dev
```

## Troubleshooting

### "Build failed"
- Check GitHub Actions logs for error details
- Ensure `npm run build:all` works locally first
- Verify Node.js version is 18+

### "WASM module not found"
- Run `cd wasm && bash build.sh` locally to generate `pkg/`
- Check that `transmitwave-wasm` is in `web/node_modules/`
- Restart dev server if running locally

### "Pages project not found"
- Verify project name matches: `wrangler pages project list`
- Create project first in dashboard if missing

## Next Steps

- [Full deployment guide](CLOUDFLARE_DEPLOYMENT.md)
- [Cloudflare Pages docs](https://developers.cloudflare.com/pages/)
- [Custom domain setup](https://developers.cloudflare.com/pages/platform/domains/)

## Local Development

```bash
# First time: build WASM once
cd wasm && bash build.sh && cd ..

# Start dev server (you'll run your vite http server)
cd web && npm run dev
```

The Vite dev server will automatically use the WASM module in `wasm/pkg/`.

---

✨ Your app is now ready for Cloudflare Pages!
