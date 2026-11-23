import { useMemo, useState } from 'react'
import type { ElementType } from 'react'
import {
  AlertCircle, Archive, ArrowDownToLine, ArrowRight, Check, CheckCircle2,
  ChevronRight, Cloud, Code2, Cpu, Database, ExternalLink,
  FileArchive, Folder, FolderOpen, HardDrive, History, Info, LayoutDashboard,
  Laptop2, ListFilter, LoaderCircle, LockKeyhole, MonitorCog, RefreshCw,
  RotateCcw, ScanLine, Search, Settings2, ShieldCheck, Terminal, X,
} from 'lucide-react'
import {
  createArchive, desktopAvailable, exportInstalledDrivers, extractArchive, inspectArchive, scanDeviceDrivers, scanInstalledApps,
  selectArchive, selectDirectory, selectSqlExports,
} from './native'
import type {
  AppScan, BackupResult, DriverExportResult, DriverScan, ExtractionResult, RestorePreview, ModuleId, Section,
  Verification,
} from './types'
import './App.css'

type NavEntry = { key: Section; text: string; icon: ElementType; group?: string }
const nav: NavEntry[] = [
  { key: 'overview', text: 'Overview', icon: LayoutDashboard, group: 'WORKSPACE' },
  { key: 'backup', text: 'Create backup', icon: Archive },
  { key: 'applications', text: 'Applications', icon: ListFilter },
  { key: 'drivers', text: 'Device drivers', icon: MonitorCog },
  { key: 'cloud', text: 'Cloud storage', icon: Cloud },
  { key: 'restore', text: 'Restore & verify', icon: RotateCcw, group: 'RECOVERY' },
  { key: 'history', text: 'Activity', icon: History },
  { key: 'settings', text: 'About & settings', icon: Settings2, group: 'SYSTEM' },
]

const modules: { id: ModuleId; name: string; details: string; icon: ElementType }[] = [
  { id: 'apps', name: 'Installed applications', details: 'Selected WinGet packages and manual reinstall list', icon: Laptop2 },
  { id: 'vscode', name: 'Visual Studio Code', details: 'Settings, profiles, snippets and keybindings', icon: Code2 },
  { id: 'chrome', name: 'Google Chrome', details: 'Bookmarks and available history; not sessions or passwords', icon: Folder },
  { id: 'git', name: 'Git configuration', details: 'Global Git configuration and ignore rules', icon: Terminal },
  { id: 'node', name: 'Developer tool versions', details: 'Installed Node.js, npm, PHP and Git versions', icon: Cpu },
  { id: 'projects', name: 'My project folders', details: 'Explicitly selected source folders, excluding dependencies and secrets', icon: FolderOpen },
  { id: 'sql', name: 'Existing SQL database exports', details: 'Selected .sql / .sql.gz files; no connection to MySQL or XAMPP', icon: Database },
]

type Activity = { title: string; detail: string; time: string; success: boolean }
const getError = (err: unknown) => err instanceof Error ? err.message : String(err)
const formatBytes = (value: number) => value >= 1024 * 1024
  ? (value / (1024 * 1024)).toFixed(1) + ' MB'
  : (value / 1024).toFixed(1) + ' KB'

function App() {
  const [page, setPage] = useState<Section>('overview')
  const [apps, setApps] = useState<AppScan | null>(null)
  const [drivers, setDrivers] = useState<DriverScan | null>(null)
  const [driverExport, setDriverExport] = useState<DriverExportResult | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [selectedModules, setSelectedModules] = useState<Set<ModuleId>>(new Set(['apps', 'vscode', 'chrome', 'git', 'node']))
  const [appSearch, setAppSearch] = useState('')
  const [driverSearch, setDriverSearch] = useState('')
  const [onlySupported, setOnlySupported] = useState(false)
  const [destination, setDestination] = useState('')
  const [customFolders, setCustomFolders] = useState<string[]>([])
  const [sqlFiles, setSqlFiles] = useState<string[]>([])
  const [busy, setBusy] = useState<string | null>(null)
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null)
  const [recentBackup, setRecentBackup] = useState<BackupResult | null>(null)
  const [verification, setVerification] = useState<Verification | null>(null)
  const [restorePreview, setRestorePreview] = useState<RestorePreview | null>(null)
  const [verifiedArchivePath, setVerifiedArchivePath] = useState<string | null>(null)
  const [extraction, setExtraction] = useState<ExtractionResult | null>(null)
  const [events, setEvents] = useState<Activity[]>([])
  const filteredApps = useMemo(() => (apps?.apps ?? []).filter((app) =>
    (!onlySupported || app.supported) &&
    (app.name.toLowerCase().includes(appSearch.toLowerCase()) ||
      app.id.toLowerCase().includes(appSearch.toLowerCase()))
  ), [apps, appSearch, onlySupported])
  const filteredDrivers = useMemo(() => (drivers?.devices ?? []).filter((item) =>
    item.name.toLowerCase().includes(driverSearch.toLowerCase()) ||
    item.provider.toLowerCase().includes(driverSearch.toLowerCase())
  ), [drivers, driverSearch])
  const compatibleApps = apps?.apps.filter((a) => a.supported).length ?? 0
  const addEvent = (title: string, detail: string, success = true) =>
    setEvents((prev) => [{ title, detail, time: new Date().toLocaleTimeString(), success }, ...prev])

  const toggleModule = (id: ModuleId) => {
    setSelectedModules((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }
  const toggleApp = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }
  const runScan = async (kind: 'apps' | 'drivers' | 'all' = 'all') => {
    if (!desktopAvailable || busy) return
    setBusy('Scanning this PC')
    setMessage(null)
    try {
      if (kind === 'apps' || kind === 'all') {
        const result = await scanInstalledApps()
        setApps(result)
        addEvent('Application inventory complete', result.apps.length + ' installed application records')
      }
      if (kind === 'drivers' || kind === 'all') {
        const result = await scanDeviceDrivers()
        setDrivers(result)
        addEvent('Driver inventory complete', result.devices.length + ' device and driver records')
      }
      setMessage({ text: 'System inventory refreshed successfully.', error: false })
    } catch (err) {
      setMessage({ text: getError(err), error: true })
      addEvent('Scan failed', getError(err), false)
    } finally { setBusy(null) }
  }
  const runDriverExport = async () => {
    if (!desktopAvailable || busy) return
    try {
      const folder = await selectDirectory()
      if (!folder) return
      setBusy('Exporting installed driver packages')
      setMessage(null)
      const result = await exportInstalledDrivers(folder)
      setDriverExport(result)
      addEvent('Driver package export complete', result.outputDirectory)
      setMessage({ text: result.packagesExported + ' third-party driver packages exported successfully.', error: false })
    } catch (err) {
      setMessage({ text: getError(err), error: true })
      addEvent('Driver export failed', getError(err), false)
    } finally { setBusy(null) }
  }
  const addProjectFolder = async () => {
    if (!desktopAvailable || busy) return
    try {
      const selected = await selectDirectory()
      if (!selected) return
      if (customFolders.some((folder) => folder.toLowerCase() === selected.toLowerCase())) return
      if (customFolders.length >= 8) {
        setMessage({ text: 'You can add up to 8 project folders per backup.', error: true })
        return
      }
      setCustomFolders((previous) => [...previous, selected])
      setSelectedModules((previous) => new Set(previous).add('projects'))
    } catch (err) { setMessage({ text: getError(err), error: true }) }
  }
  const removeProjectFolder = (path: string) => {
    setCustomFolders((previous) => previous.filter((folder) => folder !== path))
  }
  const addSqlExports = async () => {
    if (!desktopAvailable || busy) return
    try {
      const selected = await selectSqlExports()
      if (selected.length === 0) return
      const combined = [...sqlFiles, ...selected].filter((name, index, all) => all.findIndex((p) => p.toLowerCase() === name.toLowerCase()) === index)
      if (combined.length > 12) {
        setMessage({ text: 'Select no more than 12 SQL exports per backup.', error: true })
        return
      }
      setSqlFiles(combined)
      setSelectedModules((prev) => new Set([...prev, 'sql']))
    } catch (err) { setMessage({ text: getError(err), error: true }) }
  }
  const pickDestination = async () => {
    try {
      const folder = await selectDirectory()
      if (folder) setDestination(folder)
    } catch (err) { setMessage({ text: getError(err), error: true }) }
  }
  const runBackup = async () => {
    if (!desktopAvailable || busy) return
    if (!destination.trim()) {
      setMessage({ text: 'Choose an existing destination folder before creating a backup.', error: true })
      return
    }
    if (selectedModules.size === 0) {
      setMessage({ text: 'Select at least one backup category.', error: true })
      return
    }
    if (selectedModules.has('projects') && customFolders.length === 0) {
      setMessage({ text: 'Add at least one project folder or deselect My project folders.', error: true })
      return
    }
    if (selectedModules.has('sql') && sqlFiles.length === 0) {
      setMessage({ text: 'Choose existing .sql or .sql.gz exports or deselect the SQL module.', error: true })
      return
    }
    setBusy('Creating and verifying ZIP archive')
    setMessage(null)
    try {
      const result = await createArchive({
        destination,
        modules: Array.from(selectedModules),
        appIds: [...selectedIds],
        customFolders,
        sqlFiles: selectedModules.has('sql') ? sqlFiles : [],
      })
      setRecentBackup(result)
      addEvent('Local archive verified', result.archivePath)
      setMessage({ text: 'Backup created and SHA-256 verified. ' + result.fileCount + ' files archived.', error: false })
    } catch (err) {
      setMessage({ text: getError(err), error: true })
      addEvent('Backup failed', getError(err), false)
    } finally { setBusy(null) }
  }
  const runVerify = async () => {
    if (!desktopAvailable || busy) return
    setBusy('Verifying local archive')
    setVerification(null)
    setRestorePreview(null)
    setVerifiedArchivePath(null)
    setExtraction(null)
    try {
      const path = await selectArchive()
      if (!path) return
      // The preview command verifies SHA-256 once before reading any app metadata.
      const plan = await inspectArchive(path)
      setRestorePreview(plan)
      setVerifiedArchivePath(path)
      setVerification({ verified: true, filesChecked: plan.fileCount, errors: [] })
      addEvent('Archive verified and previewed', path)
      setMessage({ text: 'All ' + plan.fileCount + ' recorded files passed SHA-256 verification.', error: false })
    } catch (err) {
      const error = getError(err)
      setVerification({ verified: false, filesChecked: 0, errors: [error] })
      setMessage({ text: error, error: true })
      addEvent('Archive verification failed', error, false)
    }
    finally { setBusy(null) }
  }
  const runExtract = async () => {
    if (!desktopAvailable || busy || !verifiedArchivePath || !verification?.verified) return
    try {
      const folder = await selectDirectory()
      if (!folder) return
      setBusy('Extracting to a new recovery folder')
      const result = await extractArchive(verifiedArchivePath, folder)
      setExtraction(result)
      addEvent('Archive extracted', result.outputDirectory)
      setMessage({ text: 'Recovered ' + result.filesExtracted + ' files to a separate folder. Nothing was applied to Windows automatically.', error: false })
    } catch (err) {
      setMessage({ text: getError(err), error: true })
      addEvent('Extraction failed', getError(err), false)
    } finally { setBusy(null) }
  }
  const heading = nav.find((item) => item.key === page)?.text ?? 'Overview'

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">D<span>.</span></div>
          <div><strong>DevMove</strong><small>WINDOWS MIGRATION</small></div>
        </div>
        <div className="nav-list">
          {nav.map(({ key, text, icon: Icon, group }) => (
            <div key={key}>
              {group && <div className="nav-group">{group}</div>}
              <button type="button" className={'nav-button' + (page === key ? ' active' : '')}
                onClick={() => { setPage(key); setMessage(null) }}>
                <Icon size={18} strokeWidth={1.9} /><span>{text}</span>
                {page === key && <ChevronRight className="nav-chevron" size={15} />}
              </button>
            </div>
          ))}
        </div>
        <div className="sidebar-bottom">
          <div className="sidebar-note"><ShieldCheck size={19} />
            <div><strong>Local-first</strong><span>Your data stays on your PC unless you explicitly export it.</span></div>
          </div>
          <span className="build-label">EARLY DEVELOPMENT · v0.1.1</span>
        </div>
      </aside>
      <div className="main">
        <header className="topbar">
          <div className="breadcrumb">DevMove <ChevronRight size={14} /> <span>{heading}</span></div>
          <div className="topbar-right">
            <span className={'connection ' + (desktopAvailable ? 'connected' : '')}>
              <span className="status-dot" /> {desktopAvailable ? 'Desktop connected' : 'Browser preview'}
            </span>
            <div className="avatar">DM</div>
          </div>
        </header>
        <main className="content">
          {!desktopAvailable && <div className="notice"><Info size={18} />
            Native inventory and backup operations require the Tauri desktop app. This browser preview shows the interface only.
          </div>}
          {message && <div className={'notice ' + (message.error ? 'notice-error' : 'notice-success')}>
            {message.error ? <AlertCircle size={17} /> : <CheckCircle2 size={17} />}
            <span>{message.text}</span>
            <button className="close-notice" onClick={() => setMessage(null)} aria-label="Dismiss"><X size={16} /></button>
          </div>}
          {page === 'overview' && <>
            <div className="page-heading">
              <div><div className="eyebrow">MIGRATION WORKSPACE</div><h1>Start with a clear picture.</h1>
                <p>Review what's installed, choose what matters, and prepare your next Windows installation.</p></div>
              <button className="btn btn-primary" disabled={!desktopAvailable || !!busy} onClick={() => runScan('all')}>
                {busy ? <LoaderCircle className="spin" size={17} /> : <ScanLine size={17} />}
                {busy || 'Scan this PC'}
              </button>
            </div>
            <div className="stats">
              <Stat icon={Laptop2} label="Installed apps" value={apps ? String(apps.apps.length) : '—'} detail={apps ? 'Detected on this PC' : 'Scan required'} />
              <Stat icon={CheckCircle2} label="WinGet matches" value={apps ? String(compatibleApps) : '—'} detail="Reinstallable package IDs" />
              <Stat icon={MonitorCog} label="Driver records" value={drivers ? String(drivers.devices.length) : '—'} detail={drivers ? drivers.issues + ' reported device issues' : 'Not scanned'} />
              <Stat icon={Archive} label="Backups this session" value={String(events.filter((e) => e.title === 'Local archive verified').length)} detail="Verified local archives" />
            </div>
            <div className="two-col">
              <section className="panel">
                <div className="panel-head"><div><span className="section-kicker">YOUR MIGRATION</span><h2>Backup essentials</h2></div><Archive size={20} color="#7b8794" /></div>
                <p className="panel-sub">A selective, local ZIP backup is available in this early Rust build.</p>
                <div className="step-list">
                  <Step n="01" title="Review installed apps" text="Pick packages to document for a clean reinstall." ready={apps !== null} />
                  <Step n="02" title="Choose backup modules" text="Capture editor settings, developer tools and selected project folders." ready={selectedModules.size > 0} />
                  <Step n="03" title="Create and verify archive" text="Save a ZIP archive with a SHA-256 manifest." ready={recentBackup !== null} />
                </div>
                <button className="inline-link" onClick={() => setPage('backup')}>Configure backup <ArrowRight size={16} /></button>
              </section>
              <section className="panel">
                <div className="panel-head"><div><span className="section-kicker">RECOVERY READINESS</span><h2>Before you reinstall</h2></div><ShieldCheck size={20} color="#7b8794" /></div>
                <div className="checkline"><div className="mini-check"><Check size={14} /></div>
                  Store backups outside the Windows system partition.</div>
                <div className="checkline"><div className="mini-check"><Check size={14} /></div>
                  Enable Chrome Sync and VS Code Settings Sync.</div>
                <div className="checkline"><div className="mini-check"><Check size={14} /></div>
                  Export databases and test restores separately.</div>
                <div className="checkline"><div className="mini-check"><Check size={14} /></div>
                  Keep encryption keys and recovery credentials safe.</div>
                <div className="secondary-note"><LockKeyhole size={16} />
                  Browser passwords, passkeys and active login sessions are not copied.</div>
              </section>
            </div>
            <section className="panel full-width compact-panel">
              <div className="panel-head"><div><span className="section-kicker">PRODUCT DEVELOPMENT</span><h2>Built carefully, in stages</h2></div><span className="muted-label">Rust · Tauri · TypeScript</span></div>
              <div className="roadmap-row">
                <div><span className="roadmap-number">01</span><strong>Migration inventory</strong><small>Native scans and selection</small></div>
                <div><span className="roadmap-number">02</span><strong>Portable archives</strong><small>Local ZIP and verification</small></div>
                <div><span className="roadmap-number">03</span><strong>Cloud recovery</strong><small>Encrypted Drive sync · planned</small></div>
                <div><span className="roadmap-number">04</span><strong>Driver updates</strong><small>Compatibility and rollback · planned</small></div>
              </div>
            </section>
          </>}
          {page === 'backup' && <>
            <PageHead kicker="LOCAL BACKUP" title="Create a migration archive"
              desc="Select the data you need. DevMove creates a portable ZIP and verifies every recorded file before reporting success." />
            <div className="two-col backup-grid">
              <section className="panel">
                <div className="panel-head"><h2>Included in this backup</h2><span className="small-counter">{selectedModules.size} selected</span></div>
                <div className="module-list">
                  {modules.map(({ id, name, details, icon: Icon }) => (
                    <label key={id} className={'module-option ' + (selectedModules.has(id) ? 'is-selected' : '')}>
                      <input type="checkbox" checked={selectedModules.has(id)} onChange={() => toggleModule(id)} />
                      <span className="module-icon"><Icon size={19} /></span>
                      <span className="module-copy"><strong>{name}</strong><small>{details}</small></span>
                      <span className="fake-check">{selectedModules.has(id) && <Check size={13} strokeWidth={3} />}</span>
                    </label>
                  ))}
                </div>
                <div className="project-picker"><div className="project-picker-header"><div><strong>Selected project folders</strong><small>Up to 8 folders · no automatic disk-wide scan</small></div><button className="btn btn-secondary" type="button" onClick={addProjectFolder} disabled={!desktopAvailable || !!busy}><FolderOpen size={15}/> Add folder</button></div>{customFolders.length === 0 ? <p>No project folders selected.</p> : customFolders.map((folder) => <div className="project-folder" key={folder}><Folder size={17}/><span title={folder}>{folder}</span><button type="button" aria-label={'Remove ' + folder} onClick={() => removeProjectFolder(folder)}><X size={16}/></button></div>)}<p className="project-caution">Dependencies, generated files, common credentials and links are excluded. ZIP backups are not encrypted; review their contents before sharing.</p></div><div className="project-picker">
                  <div className="project-picker-header">
                    <div><strong>Existing SQL exports</strong><small>Choose up to 12 .sql or .sql.gz files</small></div>
                    <button className="btn btn-secondary" type="button" onClick={addSqlExports} disabled={!desktopAvailable || !!busy}><Database size={15}/> Add SQL file</button>
                  </div>
                  {sqlFiles.length === 0 ? <p>No SQL export files selected.</p> : sqlFiles.map((file) =>
                    <div className="project-folder" key={file}><Database size={17}/><span title={file}>{file}</span>
                      <button type="button" aria-label={'Remove ' + file} onClick={() => setSqlFiles((files) => files.filter((f) => f !== file))}><X size={16}/></button>
                    </div>
                  )}
                  <p className="project-caution">Only files that you explicitly select are copied. No MySQL connection is made. SQL files can contain private data and backups are not encrypted.</p>
                </div>
                <div className="unavailable-module"><Database size={18}/><div><strong>Live XAMPP database dump and automatic import</strong><small>Direct MySQL export and restore remain in development.</small></div><span>Planned</span></div>
              </section>
              <section className="panel right-panel">
                <div className="panel-head"><h2>Destination</h2><HardDrive color="#7b8794" size={20} /></div>
                <label htmlFor="backup-path" className="field-label">Local destination directory</label>
                <div className="browse-row">
                  <input id="backup-path" type="text" placeholder="Choose an external drive or a separate partition"
                    value={destination} onChange={(e) => setDestination(e.target.value)} />
                  <button className="btn btn-secondary" disabled={!desktopAvailable || !!busy} onClick={pickDestination}><FolderOpen size={16} /> Browse</button>
                </div>
                <div className="helper-text"><Info size={14} /> If you repartition the entire disk, a D: drive may also be erased. An external drive is safer.</div>
                <div className="separator" />
                <div className="cloud-addon"><Cloud size={19}/><div><strong>Also upload to Google Drive</strong><small>OAuth authorization, encryption and resumable upload</small></div><span className="pill-muted">Coming soon</span></div>
                <div className="separator" />
                <div className="summary-line"><span>Archive format</span><strong>ZIP · SHA-256 verified</strong></div>
                <div className="summary-line"><span>Software selection</span><strong>{selectedIds.size} chosen</strong></div>
                <div className="summary-line"><span>Project folders</span><strong>{selectedModules.has('projects') ? customFolders.length : 0} selected</strong></div>
                <div className="summary-line"><span>SQL exports</span><strong>{selectedModules.has('sql') ? sqlFiles.length : 0} selected</strong></div>
                <div className="summary-line"><span>Privacy</span><strong>Local, unencrypted</strong></div>
                <div className="warning-plain"><AlertCircle size={16}/> ZIP archives are not encrypted in this build. Do not upload them publicly or store sensitive data without protection.</div>
                <button className="btn btn-primary btn-full" disabled={!desktopAvailable || !!busy || !destination.trim()} onClick={runBackup}>
                  {busy ? <LoaderCircle size={17} className="spin" /> : <FileArchive size={17}/>}
                  {busy || 'Create and verify backup'}
                </button>
                {recentBackup && <><div className="backup-result">
                  <CheckCircle2 size={18}/><div><strong>Last archive verified</strong><small>{recentBackup.fileCount} files · {formatBytes(recentBackup.bytesWritten)}</small><span title={recentBackup.archivePath}>{recentBackup.archivePath}</span></div>
                </div>{recentBackup.warnings.length > 0 && <div className="backup-warnings"><AlertCircle size={16}/><div><strong>Backup completed with exclusions or warnings</strong>{recentBackup.warnings.map((warning, i) => <p key={i}>{warning}</p>)}</div></div>}</>}
                <button className="inline-link" onClick={() => setPage('applications')}>Review individual applications <ArrowRight size={16}/></button>
              </section>
            </div>
          </>}
          {page === 'applications' && <>
            <PageHead kicker="SOFTWARE INVENTORY" title="Choose the apps worth keeping"
              desc="Review your installed software. Supported packages can be restored later through WinGet; other apps are saved as a manual reinstall list."
              action={<button className="btn btn-primary" disabled={!desktopAvailable || !!busy} onClick={() => runScan('apps')}><RefreshCw size={16} className={busy ? 'spin' : ''}/>{busy || 'Scan installed apps'}</button>} />
            <section className="panel table-panel">
              <div className="toolbar"><div className="search-box"><Search size={17}/>
                <input aria-label="Search installed apps" value={appSearch} onChange={(e) => setAppSearch(e.target.value)} placeholder="Search applications or package IDs..." /></div>
                <label className="filter-check"><input type="checkbox" checked={onlySupported} onChange={(e) => setOnlySupported(e.target.checked)}/> WinGet only</label>
                <span className="record-count">{filteredApps.length} results</span>
              </div>
              <div className="selection-bar"><span>{selectedIds.size} selected</span>
                <button onClick={() => setSelectedIds((previous) => new Set([...previous, ...filteredApps.map((a) => a.id)]))} disabled={!apps}>Select visible</button>
                <button onClick={() => setSelectedIds(new Set())} disabled={selectedIds.size === 0}>Clear selection</button>
                <button className="selection-continue" onClick={() => setPage('backup')}>Continue to backup <ArrowRight size={15}/></button>
              </div>
              <div className="list-heading"><span>APPLICATION</span><span>VERSION</span><span>REINSTALL METHOD</span></div>
              <div className="app-list">
                {!apps && <EmptyState icon={ScanLine} title="No scan results yet" desc="Scan this PC to discover WinGet matches and applications requiring manual installation." />}
                {apps && filteredApps.length === 0 && <EmptyState icon={Search} title="No matching applications" desc="Try a different search or clear your filters." />}
                {filteredApps.map((app) => (
                  <label key={app.id} className="app-row">
                    <input type="checkbox" checked={selectedIds.has(app.id)} onChange={() => toggleApp(app.id)} />
                    <span className="app-symbol">{app.name.charAt(0).toUpperCase()}</span>
                    <span className="app-identity"><strong>{app.name}</strong><small>{app.supported ? app.id : 'Not matched to a WinGet package'}</small></span>
                    <span className="app-version">{app.version || '—'}</span>
                    <span className={'source-chip ' + (app.supported ? 'chip-green' : '')}>{app.supported ? 'WinGet' : 'Manual'}</span>
                  </label>
                ))}
              </div>
              {apps?.warnings?.length ? <p className="table-warning"><AlertCircle size={14}/>{apps.warnings.join('; ')}</p> : null}
              <div className="table-footer">A package list documents software for reinstallation; it does not copy application binaries or license activations.</div>
            </section>
          </>}
          {page === 'drivers' && <>
            <PageHead kicker="DEVICE HEALTH" title="Driver diagnostics"
              desc="Inspect installed drivers, identify devices reporting problems, and export available third-party packages. No driver downloads or installations."
              action={<div className="driver-actions"><button className="btn btn-secondary" disabled={!desktopAvailable || !!busy} onClick={runDriverExport}><ArrowDownToLine size={17}/> Export driver packages</button><button className="btn btn-primary" disabled={!desktopAvailable || !!busy} onClick={() => runScan('drivers')}><ScanLine size={17}/>{busy || 'Scan drivers'}</button></div>} />
            {driverExport && <div className="notice notice-success"><CheckCircle2 size={18}/><span>{driverExport.packagesExported} packages exported to {driverExport.outputDirectory}. No installed drivers were changed.</span></div>}
            <div className="stats drivers-stats">
              <Stat icon={MonitorCog} label="Signed driver records" value={drivers ? String(drivers.devices.length) : '—'} detail="Detected by Windows" />
              <Stat icon={AlertCircle} label="Devices reporting issues" value={drivers ? String(drivers.issues) : '—'} detail="Windows PnP status" />
              <Stat icon={ShieldCheck} label="Driver updates" value="Not enabled" detail="Requires verified compatibility checks" />
            </div>
            <section className="panel table-panel">
              <div className="toolbar"><div className="search-box"><Search size={17}/><input aria-label="Search driver list"
                value={driverSearch} onChange={(e) => setDriverSearch(e.target.value)} placeholder="Search devices or driver providers..." /></div>
                <span className="record-count">{filteredDrivers.length} driver records</span>
              </div>
              <div className="driver-list-heading"><span>DEVICE</span><span>PROVIDER</span><span>VERSION</span><span>STATUS</span></div>
              <div className="driver-list">
                {!drivers && <EmptyState icon={MonitorCog} title="Driver inventory not scanned" desc="Use Scan drivers to inspect your local hardware and report device issues." />}
                {drivers && filteredDrivers.length === 0 && <EmptyState icon={Search} title="No drivers match" desc="Clear the search to view the full inventory." />}
                {filteredDrivers.map((device, index) => (
                  <div className="driver-row" key={device.deviceId + '_' + index}>
                    <div className="driver-title"><span className="device-icon"><Cpu size={17}/></span><span><strong>{device.name}</strong><small>{device.manufacturer || 'Manufacturer unavailable'}</small></span></div>
                    <div className="driver-cell">{device.provider || '—'}</div>
                    <div className="driver-cell mono">{device.version || '—'}</div>
                    <div><span className={'source-chip ' + (device.problemCode ? 'chip-red' : 'chip-green')}>{device.problemCode === 28 ? 'Missing driver' : device.problemCode ? 'Issue ' + device.problemCode : 'Reported OK'}</span></div>
                  </div>
                ))}
              </div>
              <div className="table-footer">A newer version number alone does not prove an update is compatible. Windows Code 28 typically indicates a missing driver. Driver export is read-only; automatic installation and outdated-driver detection are not yet available.</div>
            </section>
          </>}
          {page === 'cloud' && <>
            <PageHead kicker="CLOUD BACKUP" title="Google Drive integration"
              desc="Keep a separate encrypted off-device copy, with explicit authorization and reliable recovery." />
            <section className="panel cloud-panel"><div className="cloud-circle"><Cloud size={32}/></div>
              <h2>Cloud backup is on the roadmap.</h2>
              <p>Google OAuth sign-in, a dedicated Dev Backup folder, encrypted transfers, resumable uploads, and remote restore browsing are being designed. Connection is not available in this build.</p>
              <div className="cloud-flow"><span><LockKeyhole size={18}/> Authorize</span><ArrowRight size={16}/><span><FileArchive size={18}/> Encrypt</span><ArrowRight size={16}/><span><Cloud size={18}/> Upload</span></div>
              <span className="subtle-hint">Your local backups are not uploaded or shared.</span>
            </section>
          </>}
          {page === 'restore' && <>
            <PageHead kicker="RECOVERY" title="Verify before restoring"
              desc="Confirm that a DevMove archive is readable and that every recorded file passes its SHA-256 integrity check."
              action={<button className="btn btn-primary" disabled={!desktopAvailable || !!busy} onClick={runVerify}><ShieldCheck size={17}/>{busy || 'Choose ZIP to verify'}</button>} />
            <section className="panel restore-panel"><div className="restore-symbol"><ArrowDownToLine size={27}/></div>
              <div><h2>Archive integrity check</h2>
                <p>Choose an archive created with DevMove. Verification does not modify files on this PC.</p>
                {verification?.verified && <button type="button" className="btn btn-secondary" disabled={!!busy} onClick={runExtract}><FolderOpen size={16}/>Extract to a new folder</button>}
                {extraction && <div className="backup-result"><CheckCircle2 size={18}/><div><strong>{extraction.filesExtracted} files extracted</strong><small>{formatBytes(extraction.bytesWritten)} · No Windows settings changed</small><span title={extraction.outputDirectory}>{extraction.outputDirectory}</span></div></div>}
                {verification && <div className={'verify-outcome ' + (verification.verified ? '' : 'verify-fail')}>
                  {verification.verified ? <CheckCircle2 size={20}/> : <AlertCircle size={20}/>}
                  {verification.verified ? verification.filesChecked + ' files verified successfully.' : verification.errors.join('; ')}
                </div>}
              </div>
            </section>
            {restorePreview && <section className="panel restore-preview">
              <div className="panel-head"><div><span className="section-kicker">READ-ONLY RESTORE PLAN</span><h2>Inside this verified backup</h2></div><ShieldCheck size={20} color="#718ba0"/></div>
              <div className="restore-plan-stats">
                <div><span>Created</span><strong>{new Date(restorePreview.createdAt).toLocaleString()}</strong></div>
                <div><span>Recorded files</span><strong>{restorePreview.fileCount}</strong></div>
                <div><span>Uncompressed size</span><strong>{formatBytes(restorePreview.totalBytes)}</strong></div>
                <div><span>Backup modules</span><strong>{restorePreview.modules.join(', ') || 'None'}</strong></div>
              </div>
              <div className="restore-plan-cols">
                <div className="restore-plan-col"><h3><CheckCircle2 size={17}/> WinGet package IDs <span>{restorePreview.wingetPackageIds.length}</span></h3>
                  <p>Recorded package IDs for a future guided reinstall. Their availability on a new PC has not been verified.</p>
                  {restorePreview.wingetPackageIds.length === 0 ? <p className="restore-empty">No WinGet package IDs found in this archive.</p>
                    : <div className="restore-package-list">{restorePreview.wingetPackageIds.slice(0, 10).map((id) => <div key={id} className="restore-package">{id}</div>)}{restorePreview.wingetPackageIds.length > 10 && <small>+ {restorePreview.wingetPackageIds.length - 10} additional IDs</small>}</div>}
                </div>
                <div className="restore-plan-col"><h3><ListFilter size={17}/> Manual reinstall list <span>{restorePreview.manualInstallations.length}</span></h3>
                  <p>These entries require manual review or a separate installer. No automatic installation occurs.</p>
                  {restorePreview.manualInstallations.length === 0 ? <p className="restore-empty">No manual installations listed.</p>
                    : <div className="restore-package-list">{restorePreview.manualInstallations.slice(0, 10).map((name, i) => <div key={i} className="restore-package">{name}</div>)}{restorePreview.manualInstallations.length > 10 && <small>+ {restorePreview.manualInstallations.length - 10} additional entries</small>}</div>}
                </div>
              </div>
              {restorePreview.sqlExports.length > 0 && <div className="restore-plan-col sql-preview">
                <h3><Database size={17}/> SQL export files <span>{restorePreview.sqlExports.length}</span></h3>
                <p>These are existing database exports copied into the ZIP. They can be extracted for manual import, but DevMove does not verify or restore their database contents.</p>
                <div className="restore-package-list">
                  {restorePreview.sqlExports.map((path) => <div key={path} className="restore-package">{path}</div>)}
                </div>
              </div>}
              {restorePreview.warnings.length > 0 && <div className="backup-warnings"><AlertCircle size={16}/><div><strong>Archive notes and exclusions</strong>{restorePreview.warnings.map((warning, i) => <p key={i}>{warning}</p>)}</div></div>}
            </section>}
            <div className="notice muted-notice"><Info size={18}/> This plan is read-only. You can extract to a new staging folder, but no apps are installed and no existing Windows settings are changed. Full automated restore and database import are still in development.</div>
          </>}
          {page === 'history' && <>
            <PageHead kicker="CURRENT SESSION" title="Activity log" desc="A record of scans, backups and checks performed during this app session. Persistent history is planned." />
            <section className="panel history-panel">{events.length === 0
              ? <EmptyState icon={History} title="No activity yet" desc="Your completed operations will appear here until you close the app." />
              : events.map((event, i) => <div key={i} className="history-row"><span className={'history-icon ' + (!event.success ? 'history-error' : '')}>{event.success ? <Check size={18}/> : <AlertCircle size={18}/>}</span>
                  <div><strong>{event.title}</strong><small>{event.detail}</small></div><time>{event.time}</time></div>)}</section>
          </>}
          {page === 'settings' && <>
            <PageHead kicker="APPLICATION" title="About DevMove" desc="An independent Windows migration project under active development." />
            <section className="panel settings-panel"><div className="settings-section"><span>Application</span><strong>DevMove Preview 0.1.1</strong></div>
              <div className="settings-section"><span>Framework</span><strong>Rust · Tauri 2 · React · TypeScript</strong></div>
              <div className="settings-section"><span>Default storage</span><strong>Local folder of your choice</strong></div>
              <div className="settings-section"><span>Backup format</span><strong>ZIP, SHA-256 manifest</strong></div>
              <div className="settings-section"><span>Cloud service</span><strong>Not connected (planned)</strong></div>
              <a className="project-link" href="https://github.com/rijonrrb/DevMove" target="_blank" rel="noreferrer">View source on GitHub <ExternalLink size={16}/></a>
            </section>
          </>}
        </main>
        <footer className="footer"><span>DevMove · Developer preview</span><span>Back up carefully. Verify before reinstalling Windows.</span></footer>
      </div>
    </div>
  )
}

function Stat({ icon: Icon, label, value, detail }: { icon: ElementType; label: string; value: string; detail: string }) {
  return <div className="stat"><div className="stat-top"><span>{label}</span><Icon size={19} strokeWidth={1.8}/></div><strong>{value}</strong><small>{detail}</small></div>
}
function Step({ n, title, text, ready }: { n: string; title: string; text: string; ready: boolean }) {
  return <div className="step"><div className={'step-index ' + (ready ? 'step-ready' : '')}>{ready ? <Check size={16}/> : n}</div><div><strong>{title}</strong><span>{text}</span></div></div>
}
function PageHead({ kicker, title, desc, action }: { kicker: string; title: string; desc: string; action?: React.ReactNode }) {
  return <div className="page-heading"><div><div className="eyebrow">{kicker}</div><h1>{title}</h1><p>{desc}</p></div>{action}</div>
}
function EmptyState({ icon: Icon, title, desc }: { icon: ElementType; title: string; desc: string }) {
  return <div className="empty-state"><span><Icon size={25}/></span><h3>{title}</h3><p>{desc}</p></div>
}
export default App
