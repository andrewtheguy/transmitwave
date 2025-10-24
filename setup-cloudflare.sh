#!/bin/bash
# Setup script for Cloudflare Pages deployment

set -e

echo "🚀 Setting up testaudio for Cloudflare Pages deployment..."
echo ""

# Check prerequisites
echo "📋 Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not found. Please install Rust:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install Node.js 18+"
    exit 1
fi

if ! command -v wasm-pack &> /dev/null; then
    echo "⚙️  Installing wasm-pack..."
    curl https://rustwasm.org/wasm-pack/installer/init.sh -sSf | sh
fi

echo "✅ All prerequisites found"
echo ""

# Install dependencies
echo "📦 Installing Node dependencies..."
cd web
npm install
cd ..
echo "✅ Node dependencies installed"
echo ""

# Build WASM
echo "🔨 Building WASM module..."
cd wasm
bash build.sh
cd ..
echo "✅ WASM module built"
echo ""

# Build web app
echo "🌐 Building web application..."
cd web
npm run build
cd ..
echo "✅ Web application built"
echo ""

# Summary
echo "✨ Setup complete!"
echo ""
echo "📁 Build output: web/dist/"
echo ""
echo "Next steps:"
echo "1. Install Wrangler CLI: npm install -g wrangler"
echo "2. Authenticate: wrangler login"
echo "3. Deploy: wrangler pages deploy web/dist"
echo ""
echo "Or connect to GitHub for automatic CI/CD:"
echo "1. Push to GitHub: git push origin main"
echo "2. Go to Cloudflare Pages dashboard"
echo "3. Create new project and connect GitHub repo"
echo "4. Set build command: npm run build:all"
echo "5. Set output directory: web/dist"
echo ""
