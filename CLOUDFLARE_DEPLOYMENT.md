# Cloudflare Pages Deployment Guide

This guide explains how to deploy the transmitwave project to Cloudflare Pages with automatic WASM compilation.

## Prerequisites

- Cloudflare account with Pages enabled
- GitHub repository connected to Cloudflare Pages
- Rust toolchain (for local builds)
- Node.js 18+ installed
- `wasm-pack` tool

## Quick Start

### 1. Install Dependencies Locally

```bash
# Install wasm-pack (one-time)
curl https://rustwasm.org/wasm-pack/installer/init.sh -sSf | sh

# Install Node dependencies
cd web
npm install
```

### 2. Local Development

```bash
# Build WASM once
cd wasm
bash build.sh

# In another terminal, start dev server
cd web
npm run dev
```

### 3. Local Production Build

```bash
# From project root
cd web
npm run build:all
```

## Manual Deployment via GitHub Actions (On-Demand)

### Setup

1. **Create Cloudflare API Token**
   - Go to Cloudflare Dashboard → Account → API Tokens
   - Create token with permissions:
     - `Account` → `Cloudflare Pages` → `Edit`
   - Copy the token

2. **Add GitHub Secrets**
   - Go to GitHub repo → Settings → Secrets and variables → Actions
   - Add two secrets:
     - `CLOUDFLARE_API_TOKEN`: Your API token from step 1
     - `CLOUDFLARE_ACCOUNT_ID`: Your Cloudflare Account ID (found in Cloudflare Dashboard)

3. **GitHub Workflow**
   - The workflow file `.github/workflows/deploy.yml` is already set up
   - Manual trigger only: Go to Actions tab → Deploy workflow → "Run workflow"
   - When triggered, it will:
     1. Install Rust and wasm-pack
     2. Build the WASM module (`wasm/pkg/`)
     3. Build the web app (`web/dist/`)
     4. Deploy to Cloudflare Pages

### Trigger Deployment

- Go to GitHub repo → Actions tab
- Click "Deploy to Cloudflare Pages" workflow
- Click "Run workflow" dropdown
- Click green "Run workflow" button
- Wait 5-10 minutes for deployment to complete

### Monitoring Deployments

- Check GitHub Actions tab for build status
- Cloudflare Pages dashboard shows deployment history
- Each deployment gets a unique preview URL

## Manual Deployment via Wrangler CLI

If you prefer manual deployments:

```bash
# Install Wrangler CLI
npm install -g wrangler

# Build everything
cd web
npm run build:all

# Deploy
wrangler pages deploy web/dist
```

## Build Output Structure

After `npm run build:all`:

```
web/dist/
├── index.html
├── assets/
│   ├── main.[hash].js      (React app)
│   ├── transmitwave-wasm_bg.wasm  (WASM module)
│   └── [other assets]
```

## Environment Configuration

### Cloudflare Pages Settings

In Cloudflare Dashboard → Pages → transmitwave → Settings:

1. **Build Configuration**
   - Build command: `npm run build:all` (if not using GitHub Actions)
   - Build output directory: `web/dist`
   - Root directory: `.` (or leave blank)

2. **Environment Variables** (optional)
   - Add any secrets needed by your app

3. **Node.js Version**
   - Set to 18.x or higher

## Troubleshooting

### WASM Module Not Loading

- Check browser DevTools → Network tab for `.wasm` file
- Ensure `vite-plugin-wasm` is properly configured in `vite.config.ts`
- Verify WASM MIME type is set to `application/wasm`

### Build Failures in GitHub Actions

Common issues:

1. **"wasm-pack not found"**
   - Ensure Rust toolchain and wasm-pack are installed in workflow
   - Check `.github/workflows/deploy.yml` has the installation steps

2. **"Module not found: transmitwave-wasm"**
   - Run `npm run build:wasm` before `npm run build`
   - Verify `wasm/pkg/` directory exists after WASM build

3. **Node version issues**
   - Ensure Node 18+ in workflow and Cloudflare Pages settings

### Local Build Issues

```bash
# Clean and rebuild everything
rm -rf web/dist wasm/pkg web/node_modules
cd web
npm install
npm run build:all
```

## Performance Optimization

The WASM module is already optimized with:
- Link Time Optimization (LTO) enabled
- Code generation unit = 1
- Size optimization (`opt-level = "z"`)

For smaller bundles, the WASM module size should be ~100-200KB gzipped.

## File Structure

```
transmitwave/
├── .github/workflows/
│   └── deploy.yml           (CI/CD pipeline)
├── web/                      (React frontend)
│   ├── src/
│   ├── public/
│   ├── dist/                (Build output - deployed)
│   ├── vite.config.ts
│   └── package.json
├── wasm/                     (Rust WASM module)
│   ├── src/
│   ├── pkg/                 (Build output - used by web)
│   ├── build.sh
│   └── Cargo.toml
├── core/                     (Rust core library)
│   ├── src/
│   └── Cargo.toml
├── CLOUDFLARE_DEPLOYMENT.md (this file)
└── wrangler.toml           (Wrangler config)
```

## First-Time Setup Steps

1. Push project to GitHub
2. Go to Cloudflare Pages → Create application
3. Connect GitHub repository
4. Set build configuration:
   - Framework: None
   - Build command: `npm install -g wasm-pack && npm run build:all`
   - Build output directory: `web/dist`
   - Root directory: `/`
5. Add environment secrets (API token, Account ID)
6. Save and deploy

Your app will be available at `transmitwave.[your-domain].pages.dev`

## Additional Resources

- [Cloudflare Pages Docs](https://developers.cloudflare.com/pages/)
- [Wasm-pack Book](https://rustwasm.org/docs/wasm-pack/)
- [Vite Documentation](https://vitejs.dev/)
