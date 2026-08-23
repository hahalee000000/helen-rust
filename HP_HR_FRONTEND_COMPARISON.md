# HP vs HR Frontend Comparison Report

**Date**: 2026-08-23  
**HP**: Rust agent (port 8001) - Built frontend  
**HR**: Python agent (port 5173) - Vite dev server

---

## 🔴 Critical Differences

### 1. Authentication Token Injection

**HR (Python)** ✅
```html
<script>window.__HELEN_TOKEN__="gOnc3bxYDcCX5NyKfGmxXpSo1lHs1_7mNRXtpFYZFPw";</script>
```
- Token injected in `index.html`
- Bootstrap logic in `main.tsx` reads and sets token
- All API calls authenticated automatically

**HP (Rust)** ❌
```html
<!-- NO token injection -->
```
- No token in `index.html`
- No bootstrap logic
- **All API calls will fail with 401 Unauthorized**

**Impact**: HP frontend is completely broken without authentication

---

### 2. Routes & Pages

| Route | HP (Rust) | HR (Python) | Notes |
|-------|-----------|-------------|-------|
| `/` | ✅ ChatPage | ✅ ChatPage | Both have chat |
| `/settings` | ✅ SettingsPage | ✅ SettingsPage | Both have settings |
| `/transcript` | ✅ TranscriptList | ❌ Missing | HP-only feature |
| `/transcript/:sessionId` | ✅ TranscriptDetail | ❌ Missing | HP-only feature |

**HP has 2 extra pages** that HR doesn't have!

---

### 3. Sidebar Navigation

**HP (Rust)**
```typescript
navItems = [
  { path: '/', label: '聊天', icon: MessageSquare },
  { path: '/transcript', label: '会话记录', icon: FileText },  // ← Extra!
  { path: '/settings', label: '设置', icon: Settings },
]
```

**HR (Python)**
```typescript
navItems = [
  { path: '/', label: '聊天', icon: MessageSquare },
  { path: '/settings', label: '设置', icon: Settings },
]
```

**Difference**: HP has "会话记录" (Transcript) link in sidebar

---

### 4. Favicon

**HP (Rust)**
```html
<link rel="icon" type="image/svg+xml" href="/vite.svg" />
```
- Uses default Vite SVG favicon
- File: `frontend/vite.svg`

**HR (Python)**
```html
<link rel="icon" type="image/png" href="/favicon.png" />
```
- Uses custom PNG favicon
- File: `frontend/public/favicon.png`

---

### 5. Logo

**HP (Rust)**
```typescript
<img src="/helen-logo-64.png" alt="Helen" className="w-10 h-10 rounded-lg" />
```
- References `/helen-logo-64.png`
- **File may not exist** (not in frontend directory)

**HR (Python)**
```typescript
<img src="/helen-logo-64.png" alt="Helen" className="w-10 h-10 rounded-lg" />
```
- Same reference
- File exists in `frontend/public/helen-logo-64.png`

---

## 🟡 Minor Differences

### 6. Build Mode

**HP (Rust)**
- Production build (`vite build`)
- Minified JS: `/assets/index-W6tXyWUK.js`
- CSS: `/assets/index-7WkXYSBs.css`
- No hot reload

**HR (Python)**
- Development mode (`vite dev`)
- Source files: `/src/main.tsx`
- React Refresh enabled
- Hot reload active

---

### 7. HTML Structure

**HP (Rust)**
```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Helen Web UI</title>
    <script type="module" crossorigin src="/assets/index-W6tXyWUK.js"></script>
    <link rel="stylesheet" crossorigin href="/assets/index-7WkXYSBs.css">
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
```

**HR (Python)**
```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <script type="module">import { injectIntoGlobalHook } from "/@react-refresh";
injectIntoGlobalHook(window);
window.$RefreshReg$ = () => {};
window.$RefreshSig$ = () => (type) => type;</script>

    <script type="module" src="/@vite/client"></script>

    <meta charset="UTF-8" />
    <link rel="icon" type="image/png" href="/favicon.png" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Helen Web UI</title>
  <script>window.__HELEN_TOKEN__="gOnc3bxYDcCX5NyKfGmxXpSo1lHs1_7mNRXtpFYZFPw";</script></head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

---

## 📊 Feature Comparison Matrix

| Feature | HP (Rust) | HR (Python) | Status |
|---------|-----------|-------------|--------|
| Chat Page | ✅ | ✅ | Aligned |
| Settings Page | ✅ | ✅ | Aligned |
| Transcript List | ✅ | ❌ | HP-only |
| Transcript Detail | ✅ | ❌ | HP-only |
| Auth Token | ❌ | ✅ | **HP broken** |
| Sidebar Nav (3 items) | ✅ | ✅ (2 items) | Different |
| Favicon | vite.svg | favicon.png | Different |
| Logo | helen-logo-64.png | helen-logo-64.png | Same |
| i18n Support | ✅ | ✅ | Aligned |
| WebSocket Chat | ✅ | ✅ | Aligned |
| File Upload | ✅ | ✅ | Aligned |
| Agent Status | ✅ | ✅ | Aligned |

---

## 🔧 Required Fixes for HP

### Priority 1: Authentication (CRITICAL)

**Problem**: HP frontend has no auth token, all API calls fail

**Solution**:
1. Add token injection to `frontend/index.html`:
```html
<script>window.__HELEN_TOKEN__="gOnc3bxYDcCX5NyKfGmxXpSo1lHs1_7mNRXtpFYZFPw";</script>
```

2. Or implement token bootstrap in Rust server:
   - Read token from config
   - Inject into HTML before serving

---

### Priority 2: Missing Assets

**Problem**: HP references files that don't exist

**Solution**:
1. Copy `favicon.png` from HR to HP
2. Copy `helen-logo-64.png` from HR to HP
3. Update `index.html` to use correct favicon

---

### Priority 3: Transcript Pages (Optional)

**Decision**: Should HR have transcript pages too?

**Option A**: Add transcript pages to HR
- Port TranscriptList and TranscriptDetail components
- Add `/transcript` route to App.tsx
- Add sidebar link

**Option B**: Remove transcript pages from HP
- Remove `/transcript` routes from App.tsx
- Remove sidebar link
- Keep HP and HR aligned

---

## 📝 Summary

**HP (Rust) has 3 critical issues:**
1. ❌ No authentication token → Frontend completely broken
2. ❌ Missing favicon.png → Shows default Vite icon
3. ❌ Missing helen-logo-64.png → Logo may not load

**HP has 2 extra features:**
1. ✅ Transcript list page
2. ✅ Transcript detail page

**Recommendation:**
1. Fix authentication first (CRITICAL)
2. Copy missing assets from HR
3. Decide whether to keep or remove transcript pages
4. Rebuild frontend with `npm run build`
5. Restart HP server

---

## 🧪 Verification Commands

```bash
# Check if HP frontend loads
curl -s http://127.0.0.1:8001/ | grep "__HELEN_TOKEN__"

# Check if favicon exists
curl -I http://127.0.0.1:8001/favicon.png

# Check if logo exists
curl -I http://127.0.0.1:8001/helen-logo-64.png

# Test API with token
curl -H "X-Helen-Token: gOnc3bxYDcCX5NyKfGmxXpSo1lHs1_7mNRXtpFYZFPw" \
  http://127.0.0.1:8001/api/status
```
