# Cloudflare Pages Deployment Checklist

Use this checklist to ensure everything is ready for deployment.

## Pre-Deployment Setup

- [ ] **Local Build Test**
  ```bash
  cd /Users/it3/codes/andrew/testaudio
  cd web && npm run build:all
  ```
  Expected: `web/dist/` folder created with HTML, JS, CSS, and WASM files

- [ ] **WASM Module Built**
  ```bash
  ls -la wasm/pkg/
  ```
  Expected: See `testaudio_wasm_bg.wasm` file

- [ ] **Git Status Clean**
  ```bash
  git status
  ```
  Expected: No uncommitted changes (after build artifacts are ignored)

## Choose Deployment Method

### Method 1: GitHub Actions (Automatic) ⭐ RECOMMENDED

- [ ] **Have Cloudflare Account**
  - Visit https://dash.cloudflare.com/

- [ ] **Create Cloudflare API Token**
  - Go to https://dash.cloudflare.com/profile/api-tokens
  - Create token with "Edit Cloudflare Pages" template
  - Copy token (save somewhere safe)

- [ ] **Get Cloudflare Account ID**
  - https://dash.cloudflare.com/
  - Account ID shown in bottom-left corner
  - Copy this ID

- [ ] **Add GitHub Secrets**
  - Go to GitHub repo → Settings → Secrets and variables → Actions
  - Click "New repository secret"
  - Create secret: `CLOUDFLARE_API_TOKEN` = (your token)
  - Create secret: `CLOUDFLARE_ACCOUNT_ID` = (your account ID)
  - Verify both secrets are created

- [ ] **Commit and Push**
  ```bash
  git add .
  git commit -m "Add Cloudflare Pages deployment configuration"
  git push origin main
  ```

- [ ] **Verify GitHub Actions**
  - Go to GitHub repo → Actions tab
  - Should see "Deploy to Cloudflare Pages" workflow running
  - Wait for green checkmark (5-10 minutes)

- [ ] **Check Cloudflare Dashboard**
  - https://dash.cloudflare.com/
  - Go to Pages → testaudio
  - Should see successful deployment
  - Copy the deployment URL

### Method 2: Cloudflare Pages Dashboard

- [ ] **Push to GitHub**
  ```bash
  git push origin main
  ```

- [ ] **Go to Cloudflare Pages**
  - https://dash.cloudflare.com/
  - Pages → Create application

- [ ] **Connect GitHub**
  - Select "Connect to Git"
  - Authorize GitHub
  - Select `testaudio` repository

- [ ] **Configure Build**
  - Production branch: `main`
  - Build command: `npm run build:all`
  - Build output directory: `web/dist`
  - Root directory: (leave blank or `.`)

- [ ] **Set Secrets (Optional)**
  - Add any environment variables if needed
  - Leave empty if using defaults

- [ ] **Deploy**
  - Click "Save and Deploy"
  - Cloudflare builds and deploys automatically
  - Wait for success (5-10 minutes)

### Method 3: Wrangler CLI

- [ ] **Install Wrangler**
  ```bash
  npm install -g wrangler
  ```

- [ ] **Authenticate Wrangler**
  ```bash
  wrangler login
  ```
  Expected: Browser opens for authentication

- [ ] **Local Build**
  ```bash
  cd web
  npm run build:all
  ```
  Expected: `web/dist/` created

- [ ] **Deploy**
  ```bash
  wrangler pages deploy web/dist --project-name=testaudio
  ```
  Expected: Deployment URL printed to console

## Post-Deployment Verification

- [ ] **Site is Live**
  - Visit deployment URL
  - Should see audio control interface
  - No 404 errors

- [ ] **WASM Module Loads**
  - Open browser DevTools (F12)
  - Go to Network tab
  - Refresh page
  - Should see `.wasm` file loading successfully
  - Check Console tab for no errors

- [ ] **Functionality Works**
  - Try recording audio (browser will ask for permission)
  - Play back recording
  - Check frequency detection works
  - Test UI interactions

- [ ] **Performance Acceptable**
  - Page loads within 3 seconds
  - WASM module loads within 2 seconds
  - No network errors in DevTools

- [ ] **Monitor Dashboard**
  - Cloudflare Pages dashboard shows successful deployment
  - Check analytics if available
  - Verify SSL/TLS is active

## Ongoing Operations

- [ ] **Set Up Monitoring**
  - Check Cloudflare Pages dashboard weekly
  - Monitor deployment history
  - Set up alerts if available

- [ ] **Automatic Deployments**
  - Any push to `main` deploys automatically (if using GitHub Actions)
  - Each deployment creates unique URL for preview
  - Production URL stays consistent

- [ ] **Rollback Plan**
  - Know how to rollback: Cloudflare Pages dashboard → Previous deployment → Rollback
  - Test rollback process if needed

## Troubleshooting Checklist

If deployment fails, check:

- [ ] **GitHub Actions Logs**
  - GitHub → Actions → Deploy workflow
  - Check error messages

- [ ] **Cloudflare Logs**
  - Cloudflare dashboard → Pages → testaudio → Deployments
  - Check deployment logs

- [ ] **Local Build Works**
  ```bash
  cd web && npm run build:all
  ```
  - If this fails locally, fix it before pushing

- [ ] **Dependencies Installed**
  - `web/package.json` dependencies latest
  - `wasm/Cargo.toml` dependencies correct
  - `core/Cargo.toml` dependencies correct

- [ ] **WASM Module Present**
  ```bash
  ls wasm/pkg/
  ```
  - Should have `testaudio_wasm_bg.wasm`

- [ ] **Secrets Configured** (if using GitHub Actions)
  - `CLOUDFLARE_API_TOKEN` is set
  - `CLOUDFLARE_ACCOUNT_ID` is set
  - Both are correct values

## Configuration Reference

### Build Command
```bash
npm run build:all
```

This runs:
1. `cd ../wasm && bash build.sh` - Builds WASM
2. `vite build` - Builds React app

### Output Directory
```
web/dist/
```

### Project Name
```
testaudio
```

### Site URL
```
https://testaudio.[your-domain].pages.dev
```
Replace `[your-domain]` with your Cloudflare Pages domain

## Quick Commands

```bash
# Test local build
cd web && npm run build:all

# Deploy with Wrangler
wrangler pages deploy web/dist --project-name=testaudio

# View deployment status
wrangler pages list --project-name=testaudio

# Clean build artifacts
rm -rf web/dist wasm/pkg web/node_modules
cd web && npm install

# View WASM size
ls -lh wasm/pkg/testaudio_wasm_bg.wasm
```

## Success Criteria

Your deployment is successful when:

✅ Site loads at deployment URL
✅ WASM module loads (check Network tab)
✅ Audio recording works
✅ No console errors
✅ Pages loads in <3 seconds
✅ All buttons and features respond
✅ SSL/TLS certificate is active

---

**Ready to deploy? Start with your chosen method above!**

Questions? Check:
- [CLOUDFLARE_QUICKSTART.md](CLOUDFLARE_QUICKSTART.md)
- [CLOUDFLARE_DEPLOYMENT.md](CLOUDFLARE_DEPLOYMENT.md)
- [DEPLOYMENT_SUMMARY.md](DEPLOYMENT_SUMMARY.md)
