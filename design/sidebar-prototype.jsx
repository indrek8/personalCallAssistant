import React, { useState, useEffect, useRef } from 'react';
import { 
  Plus, 
  Settings, 
  Circle, 
  CheckCircle2, 
  MessageSquare, 
  AlertCircle, 
  HelpCircle, 
  Calendar, 
  ChevronDown, 
  ChevronRight, 
  Clock, 
  Play, 
  Square, 
  Pause, 
  Send, 
  MoreHorizontal,
  Trash2,
  RefreshCw,
  Layout,
  Mic,
  Cpu,
  Save,
  ExternalLink,
  User,
  Hash
} from 'lucide-react';

const App = () => {
  // Navigation State
  const [currentScreen, setCurrentScreen] = useState('empty'); // 'empty', 'setup', 'live', 'analysis', 'history', 'settings'
  const [activeProject, setActiveProject] = useState('Product Design');
  const [activeSessionId, setActiveSessionId] = useState(null);
  const [isProjectExpanded, setIsProjectExpanded] = useState(true);

  // Live Session State
  const [elapsedTime, setElapsedTime] = useState(0);
  const [isRecording, setIsRecording] = useState(false);
  const scrollRef = useRef(null);

  // Mock Projects/Sessions Data
  const projects = [
    { name: 'Product Design', color: '#3b82f6', sessions: [
      { id: 's1', name: 'UI Feedback Sync', date: 'Oct 24' },
      { id: 's2', name: 'Design System RFC', date: 'Oct 22' }
    ]},
    { name: 'Investor Relations', color: '#ef4444', sessions: [
      { id: 's3', name: 'Series A Prep', date: 'Oct 20' }
    ]}
  ];

  // Helper to change screen
  const navTo = (screen, sessionId = null) => {
    setCurrentScreen(screen);
    if (sessionId) setActiveSessionId(sessionId);
    if (screen === 'live') {
      setIsRecording(true);
      setElapsedTime(0);
    } else {
      setIsRecording(false);
    }
  };

  // Timer Effect
  useEffect(() => {
    let interval;
    if (isRecording) {
      interval = setInterval(() => setElapsedTime(prev => prev + 1), 1000);
    }
    return () => clearInterval(interval);
  }, [isRecording]);

  const formatTime = (seconds) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  // --- COMPONENT: Sidebar ---
  const Sidebar = () => (
    <div className="w-[240px] h-full bg-[#1a1a1a] border-r border-[#262626] flex flex-col select-none">
      <div className="p-4">
        <button 
          onClick={() => navTo('setup')}
          className="w-full bg-white hover:bg-gray-200 text-black font-medium py-2 rounded-lg flex items-center justify-center gap-2 transition-colors"
        >
          <Plus size={16} /> New Session
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-2">
        <div className="mb-6">
          <div 
            className="flex items-center gap-2 px-2 py-1 text-xs font-semibold text-gray-500 uppercase tracking-wider cursor-pointer hover:text-gray-300"
            onClick={() => setIsProjectExpanded(!isProjectExpanded)}
          >
            {isProjectExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            Projects
          </div>
          
          {isProjectExpanded && projects.map(p => (
            <div key={p.name} className="mt-1">
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-md hover:bg-[#262626] text-sm text-gray-300 group cursor-pointer">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: p.color }}></div>
                <span className="flex-1 truncate">{p.name}</span>
              </div>
              <div className="ml-4 border-l border-[#333]">
                {p.sessions.map(s => (
                  <div 
                    key={s.id}
                    onClick={() => navTo('history', s.id)}
                    className={`flex flex-col px-4 py-2 rounded-r-md cursor-pointer text-sm ${activeSessionId === s.id && currentScreen === 'history' ? 'bg-[#262626] text-white border-l-2 border-white' : 'text-gray-500 hover:text-gray-300'}`}
                  >
                    <span className="truncate">{s.name}</span>
                    <span className="text-[10px] opacity-60 font-mono">{s.date}</span>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="p-4 border-t border-[#262626] flex items-center justify-between">
        <button className="text-gray-400 hover:text-white flex items-center gap-2 text-sm transition-colors">
          <Plus size={16} /> New Project
        </button>
        <button 
          onClick={() => navTo('settings')}
          className={`p-1.5 rounded-md hover:bg-[#262626] transition-colors ${currentScreen === 'settings' ? 'text-white bg-[#262626]' : 'text-gray-400'}`}
        >
          <Settings size={18} />
        </button>
      </div>
    </div>
  );

  // --- SCREEN 1: Empty State ---
  const EmptyState = () => (
    <div className="flex-1 flex flex-col items-center justify-center bg-[#0d0d0d] text-center p-8">
      <div className="w-16 h-16 bg-[#1a1a1a] rounded-2xl flex items-center justify-center mb-6 border border-[#262626]">
        <Mic className="text-gray-400" size={32} />
      </div>
      <h2 className="text-2xl font-semibold text-white mb-2">Your AI Wingman is ready</h2>
      <p className="text-gray-400 max-w-md mb-8">
        Capture, transcribe, and analyze your meetings in real-time. Everything stays local and private.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-2xl w-full mb-8">
        <div className="bg-[#1a1a1a] p-4 rounded-xl border border-[#262626] text-left">
          <h3 className="text-sm font-medium text-white mb-2 flex items-center gap-2">
            <CheckCircle2 size={14} className="text-green-500" /> Configure API Key
          </h3>
          <p className="text-xs text-gray-500">Connect to Claude for live intelligence and summaries.</p>
        </div>
        <div className="bg-[#1a1a1a] p-4 rounded-xl border border-[#262626] text-left">
          <h3 className="text-sm font-medium text-white mb-2 flex items-center gap-2">
            <CheckCircle2 size={14} className="text-green-500" /> Set Up Audio
          </h3>
          <p className="text-xs text-gray-500">Ensure virtual audio device is active for Teams/Zoom capture.</p>
        </div>
      </div>

      <button 
        onClick={() => navTo('setup')}
        className="px-6 py-3 bg-white text-black font-semibold rounded-lg hover:bg-gray-200 transition-colors flex items-center gap-2"
      >
        <Plus size={18} /> Create Your First Session
      </button>
    </div>
  );

  // --- SCREEN 2: Setup ---
  const SessionSetup = () => (
    <div className="flex-1 flex flex-col bg-[#0d0d0d] overflow-y-auto">
      <div className="max-w-3xl w-full mx-auto py-12 px-8">
        <h1 className="text-3xl font-semibold text-white mb-8">New Session</h1>
        
        <div className="space-y-6">
          <div className="grid grid-cols-2 gap-6">
            <div className="space-y-2">
              <label className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Project</label>
              <div className="relative">
                <select className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white appearance-none focus:outline-none focus:ring-1 focus:ring-white/20">
                  <option>Product Design</option>
                  <option>Investor Relations</option>
                  <option>+ New Project</option>
                </select>
                <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500" size={16} />
              </div>
            </div>
            <div className="space-y-2">
              <label className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Session Name</label>
              <input 
                type="text" 
                placeholder="e.g. Q4 Planning"
                className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white focus:outline-none focus:ring-1 focus:ring-white/20"
              />
            </div>
          </div>

          <div className="space-y-2">
            <label className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Context for AI</label>
            <textarea 
              rows={4}
              placeholder="Paste meeting agenda, key numbers, or background documents. The AI uses this to fact-check live conversation."
              className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white resize-none focus:outline-none focus:ring-1 focus:ring-white/20"
            />
          </div>

          <div className="space-y-2">
            <label className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Participants</label>
            <input 
              type="text" 
              placeholder="Sarah, Mike, David"
              className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white focus:outline-none focus:ring-1 focus:ring-white/20"
            />
          </div>

          <div className="space-y-3">
            <label className="text-xs font-semibold text-gray-500 uppercase tracking-wider">Enable Live Analysis</label>
            <div className="flex gap-2">
              {[
                { key: 'F', label: 'Fact-check', color: 'border-amber-500 text-amber-500' },
                { key: 'C', label: 'Commitments', color: 'border-blue-500 text-blue-500' },
                { key: 'S', label: 'Suggestions', color: 'border-green-500 text-green-500' },
                { key: 'Q', label: 'Questions', color: 'border-purple-500 text-purple-500' },
              ].map(feat => (
                <button key={feat.key} className={`flex-1 py-3 rounded-lg border bg-[#1a1a1a] flex flex-col items-center justify-center gap-1 hover:bg-[#262626] transition-colors ${feat.color}`}>
                  <span className="text-lg font-bold">{feat.key}</span>
                  <span className="text-[10px] uppercase font-semibold">{feat.label}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="pt-6 border-t border-[#262626]">
            <button 
              onClick={() => navTo('live')}
              className="w-full py-4 bg-white text-black font-bold rounded-xl hover:bg-gray-200 transition-all flex items-center justify-center gap-3 active:scale-[0.98]"
            >
              <div className="w-3 h-3 bg-red-500 rounded-full animate-pulse"></div>
              Start Live Session
            </button>
          </div>
        </div>
      </div>
    </div>
  );

  // --- SCREEN 3: Live Session ---
  const LiveSession = () => {
    const [messages] = useState([
      { time: '00:03:12', text: "Actually, we need to finalize the Figma handoff by Friday if we want the dev team to start on Monday." },
      { time: '00:04:45', text: "Sarah, can you check if the API documentation is ready for the auth endpoints?" },
      { time: '00:05:22', text: "Yes, I'll have that sent over. Also, we quoted the client $50k for this phase, right?" },
      { time: '00:06:10', text: "I believe it was $45k, but I need to double-check the contract." },
      { time: '00:06:40', text: "Wait, the contract says $45k, not $50k. I see it here." }
    ]);

    const [findings] = useState([
      { type: 'F', title: 'Budget Conflict', desc: 'Client contract shows $45,000 for phase 1.', icon: <AlertCircle size={14} />, color: 'text-amber-500 bg-amber-500/10 border-amber-500/20' },
      { type: 'C', title: 'Commitment Detected', desc: 'Sarah to send API documentation.', icon: <CheckCircle2 size={14} />, color: 'text-blue-500 bg-blue-500/10 border-blue-500/20' },
      { type: 'Q', title: 'Missing Info', desc: 'Who is handling the mobile responsive audit?', icon: <HelpCircle size={14} />, color: 'text-purple-500 bg-purple-500/10 border-purple-500/20' },
    ]);

    return (
      <div className="flex-1 flex flex-col bg-[#0d0d0d] overflow-hidden">
        {/* Live Toolbar */}
        <div className="h-12 bg-[#1a1a1a] border-b border-[#262626] flex items-center justify-between px-4">
          <div className="flex items-center gap-3">
            <div className="w-2.5 h-2.5 bg-red-500 rounded-full animate-pulse"></div>
            <span className="text-sm font-medium text-white">Board Call Q2</span>
            <span className="text-xs font-mono text-gray-500">{formatTime(elapsedTime)}</span>
          </div>
          <div className="flex items-center gap-2">
            <button className="p-1.5 hover:bg-[#262626] rounded-md text-gray-400"><Pause size={16} /></button>
            <button 
              onClick={() => navTo('analysis')}
              className="flex items-center gap-2 px-3 py-1 bg-red-500 hover:bg-red-600 text-white text-xs font-bold rounded-md transition-colors"
            >
              <Square size={12} fill="white" /> STOP
            </button>
          </div>
          <div className="text-[10px] font-mono text-gray-500">API COST: $0.12</div>
        </div>

        {/* Transcript Area */}
        <div className="flex-1 overflow-y-auto p-6 space-y-6" ref={scrollRef}>
          {messages.map((m, i) => (
            <div key={i} className="flex gap-4 group">
              <div className="w-16 pt-1 text-[10px] font-mono text-gray-600 tabular-nums">{m.time}</div>
              <div className="flex-1">
                <div className="text-[10px] font-bold text-gray-500 uppercase mb-1">Speaker</div>
                <div className="text-sm text-gray-200 leading-relaxed max-w-2xl">{m.text}</div>
              </div>
            </div>
          ))}
          <div className="flex gap-4 animate-pulse">
            <div className="w-16 pt-1 text-[10px] font-mono text-gray-700 tabular-nums">00:07:15</div>
            <div className="flex-1 space-y-2">
              <div className="h-1 bg-gray-800 w-12 rounded"></div>
              <div className="h-4 bg-gray-900 w-3/4 rounded"></div>
            </div>
          </div>
        </div>

        {/* AI Sidebar/Panel */}
        <div className="h-[40%] bg-[#1a1a1a] border-t border-[#262626] flex flex-col">
          <div className="flex items-center justify-between px-4 py-2 border-b border-[#262626]">
            <div className="flex gap-1.5">
              {['F', 'C', 'S', 'Q'].map(f => (
                <button key={f} className={`w-6 h-6 text-[10px] font-bold rounded flex items-center justify-center ${f === 'F' ? 'bg-amber-500 text-black' : 'bg-[#262626] text-gray-500'}`}>
                  {f}
                </button>
              ))}
            </div>
            <span className="text-[10px] font-bold text-gray-500 uppercase tracking-widest">Live Intelligence</span>
            <div className="w-16"></div>
          </div>

          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {findings.map((f, i) => (
              <div key={i} className={`p-3 rounded-lg border ${f.color} flex gap-3 relative group`}>
                <div className="mt-0.5">{f.icon}</div>
                <div className="flex-1">
                  <div className="text-xs font-bold mb-0.5">{f.title}</div>
                  <div className="text-xs opacity-90 leading-snug">{f.desc}</div>
                </div>
                {f.type === 'C' && (
                  <button className="text-[10px] font-bold underline opacity-0 group-hover:opacity-100 transition-opacity">
                    + SAVE
                  </button>
                )}
              </div>
            ))}
          </div>

          <div className="p-3 bg-[#0d0d0d] border-t border-[#262626]">
            <div className="relative">
              <input 
                type="text" 
                placeholder="Ask AI about the call..." 
                className="w-full bg-[#1a1a1a] border border-[#262626] rounded-full pl-4 pr-10 py-2 text-xs focus:outline-none focus:ring-1 focus:ring-white/20"
              />
              <button className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white transition-colors">
                <Send size={14} />
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  };

  // --- SCREEN 4: Post Analysis ---
  const PostAnalysis = () => {
    const [isProcessing, setIsProcessing] = useState(true);
    
    useEffect(() => {
      const timer = setTimeout(() => setIsProcessing(false), 2000);
      return () => clearTimeout(timer);
    }, []);

    if (isProcessing) {
      return (
        <div className="flex-1 flex flex-col items-center justify-center bg-[#0d0d0d]">
          <div className="w-12 h-12 border-4 border-gray-800 border-t-white rounded-full animate-spin mb-4"></div>
          <h2 className="text-xl font-medium text-white mb-1">Analyzing transcript...</h2>
          <p className="text-sm text-gray-500">Extracting actions and synthesizing decisions.</p>
        </div>
      );
    }

    return (
      <div className="flex-1 flex flex-col bg-[#0d0d0d] overflow-y-auto">
        <div className="max-w-4xl w-full mx-auto p-10 space-y-10">
          <header className="space-y-2">
            <h1 className="text-3xl font-bold text-white">Board Call Q2</h1>
            <div className="flex items-center gap-4 text-sm text-gray-500">
              <span className="flex items-center gap-1.5"><Calendar size={14} /> Oct 24, 2024</span>
              <span className="flex items-center gap-1.5"><Clock size={14} /> 42 mins</span>
              <span className="flex items-center gap-1.5"><User size={14} /> 4 Participants</span>
            </div>
          </header>

          <section className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-xs font-bold text-gray-500 uppercase tracking-widest">Executive Summary</h3>
              <button className="text-[10px] font-bold text-gray-400 hover:text-white flex items-center gap-1 uppercase">
                <RefreshCw size={10} /> Regenerate
              </button>
            </div>
            <div className="bg-[#1a1a1a] p-6 rounded-xl border border-[#262626] text-gray-300 text-sm leading-relaxed">
              The primary focus of the meeting was finalizing the phase 1 deliverables and budget alignment. 
              The team identified a $5k discrepancy in the initial budget projections versus the signed contract. 
              Key milestones for the Figma handoff were agreed upon for Friday to ensure development begins on schedule.
            </div>
          </section>

          <section className="space-y-4">
            <h3 className="text-xs font-bold text-gray-500 uppercase tracking-widest">Extracted Actions</h3>
            <div className="space-y-2">
              {[
                { title: 'Finalize Figma handoff', owner: 'Me', due: 'Oct 27', quote: '"we need to finalize the Figma handoff by Friday"' },
                { title: 'Send API documentation to team', owner: 'Sarah', due: 'Oct 25', quote: '"Sarah, can you check if the API documentation is ready..." ' },
                { title: 'Draft budget correction email', owner: 'Me', due: 'Oct 26', quote: '"I believe it was $45k, but I need to double-check..."' }
              ].map((item, i) => (
                <div key={i} className="bg-[#1a1a1a] p-4 rounded-xl border border-[#262626] flex items-start gap-4">
                  <input type="checkbox" defaultChecked className="mt-1 w-4 h-4 rounded border-[#333] bg-transparent accent-white" />
                  <div className="flex-1 space-y-2">
                    <div className="flex items-center justify-between">
                      <input 
                        defaultValue={item.title}
                        className="bg-transparent text-white text-sm font-medium focus:outline-none"
                      />
                      <div className="flex items-center gap-3">
                        <select className="bg-transparent text-[10px] font-bold text-gray-400 focus:outline-none">
                          <option>{item.owner}</option>
                          <option>Sarah</option>
                          <option>David</option>
                        </select>
                        <span className="text-[10px] font-bold text-gray-400 flex items-center gap-1 uppercase tracking-tighter">
                          <Calendar size={10} /> {item.due}
                        </span>
                        <button className="text-gray-600 hover:text-red-400"><Trash2 size={14} /></button>
                      </div>
                    </div>
                    <div className="text-[10px] font-mono text-gray-600 italic">{item.quote}</div>
                  </div>
                </div>
              ))}
              <button className="w-full py-3 border border-dashed border-[#262626] rounded-xl text-xs text-gray-500 hover:text-gray-300 hover:bg-[#1a1a1a] transition-all">
                + Add action manually
              </button>
            </div>
          </section>

          <div className="flex items-center gap-4 pt-6 border-t border-[#262626]">
            <button 
              onClick={() => navTo('empty')}
              className="px-8 py-3 bg-white text-black font-bold rounded-lg hover:bg-gray-200 transition-colors"
            >
              Save & Close
            </button>
            <button 
              onClick={() => navTo('history', 's1')}
              className="text-sm font-medium text-gray-500 hover:text-white"
            >
              Back to Full Transcript
            </button>
          </div>
        </div>
      </div>
    );
  };

  // --- SCREEN 5: History / Past Session ---
  const PastSession = () => (
    <div className="flex-1 flex flex-col bg-[#0d0d0d] overflow-hidden">
      <div className="p-8 border-b border-[#262626] bg-[#121212]">
        <div className="flex justify-between items-start mb-6">
          <div>
            <h1 className="text-2xl font-bold text-white mb-2">UI Feedback Sync</h1>
            <div className="flex items-center gap-4 text-xs text-gray-500">
              <span className="flex items-center gap-1.5 font-mono">OCT 24, 2024</span>
              <span className="flex items-center gap-1.5"><Clock size={12} /> 28 mins</span>
              <span className="flex items-center gap-1.5"><Hash size={12} /> $0.08 spent</span>
            </div>
          </div>
          <button className="flex items-center gap-2 px-3 py-1.5 bg-[#262626] hover:bg-[#333] text-white text-xs font-bold rounded-md">
            <RefreshCw size={12} /> RE-ANALYZE
          </button>
        </div>

        <div className="grid grid-cols-2 gap-8">
          <div className="space-y-3">
            <h3 className="text-[10px] font-bold text-gray-500 uppercase tracking-widest">Summary</h3>
            <p className="text-xs text-gray-400 leading-relaxed">
              Review of the mobile mockup version 2. The team agreed the navigation bar was too prominent and needs a 20% height reduction. Mike will handle the visual polish.
            </p>
          </div>
          <div className="space-y-3">
            <h3 className="text-[10px] font-bold text-gray-500 uppercase tracking-widest">Action Status</h3>
            <div className="space-y-2">
              <div className="flex items-center justify-between p-2 bg-[#1a1a1a] rounded border border-[#262626]">
                <span className="text-xs text-gray-200">Reduce Nav Height</span>
                <span className="px-2 py-0.5 rounded-full text-[9px] font-bold bg-green-500/10 text-green-500 border border-green-500/20">DONE</span>
              </div>
              <div className="flex items-center justify-between p-2 bg-[#1a1a1a] rounded border border-[#262626]">
                <span className="text-xs text-gray-200">Export assets for dev</span>
                <span className="px-2 py-0.5 rounded-full text-[9px] font-bold bg-amber-500/10 text-amber-500 border border-amber-500/20">PENDING</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-8 space-y-6">
        {[
          { time: '00:01:20', text: "Let's look at the mobile view first." },
          { time: '00:03:45', text: "I feel like this nav bar is taking up way too much real estate on smaller screens." },
          { time: '00:04:10', text: "Agree. Let's aim for a 20% reduction in height." }
        ].map((m, i) => (
          <div key={i} className="flex gap-4">
            <div className="w-16 pt-1 text-[10px] font-mono text-gray-600 tabular-nums">{m.time}</div>
            <div className="flex-1 text-sm text-gray-400 leading-relaxed">{m.text}</div>
          </div>
        ))}
      </div>
    </div>
  );

  // --- SCREEN 6: Settings ---
  const SettingsScreen = () => (
    <div className="flex-1 flex flex-col bg-[#0d0d0d] overflow-y-auto">
      <div className="max-w-2xl w-full mx-auto py-12 px-8 space-y-10">
        <h1 className="text-3xl font-semibold text-white">Settings</h1>

        <section className="space-y-4">
          <div className="flex items-center gap-2 text-white font-medium">
            <Cpu size={18} /> API Configuration
          </div>
          <div className="space-y-2">
            <label className="text-xs text-gray-500 uppercase font-bold tracking-widest">Claude API Key</label>
            <div className="relative">
              <input 
                type="password" 
                defaultValue="sk-ant-api03-................" 
                className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white focus:outline-none"
              />
              <button className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-blue-500 font-bold">SHOW</button>
            </div>
            <p className="text-[10px] text-gray-600 italic">Keys are stored locally in your system keychain.</p>
          </div>
        </section>

        <section className="space-y-4">
          <div className="flex items-center gap-2 text-white font-medium">
            <Mic size={18} /> Audio Input
          </div>
          <div className="space-y-2">
            <label className="text-xs text-gray-500 uppercase font-bold tracking-widest">Source Device</label>
            <select className="w-full bg-[#1a1a1a] border border-[#262626] rounded-lg px-4 py-2.5 text-white focus:outline-none">
              <option>BlackHole 2ch (Virtual Audio)</option>
              <option>MacBook Pro Microphone</option>
              <option>Studio Display Mic</option>
            </select>
          </div>
        </section>

        <section className="space-y-4">
          <div className="flex items-center gap-2 text-white font-medium">
            <MessageSquare size={18} /> Transcription
          </div>
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-xs text-gray-500 uppercase font-bold tracking-widest">Whisper Model</label>
              <div className="grid grid-cols-3 gap-2">
                {['Base', 'Small', 'Medium'].map(m => (
                  <button key={m} className={`py-2 rounded-lg border text-xs font-bold ${m === 'Medium' ? 'border-white bg-white text-black' : 'border-[#262626] bg-[#1a1a1a] text-gray-400'}`}>
                    {m}
                  </button>
                ))}
              </div>
              <p className="text-[10px] text-gray-600">Medium provides best accuracy but consumes more CPU.</p>
            </div>
            
            <div className="flex items-center justify-between p-3 bg-[#1a1a1a] border border-[#262626] rounded-lg">
              <div className="text-xs text-gray-300">Default to recording speaker names</div>
              <div className="w-10 h-5 bg-[#333] rounded-full relative">
                <div className="w-4 h-4 bg-gray-500 rounded-full absolute left-0.5 top-0.5"></div>
              </div>
            </div>
          </div>
        </section>

        <section className="space-y-4">
          <div className="flex items-center gap-2 text-white font-medium">
            <Layout size={18} /> Storage
          </div>
          <div className="bg-[#1a1a1a] p-4 rounded-lg border border-[#262626] flex items-center justify-between">
            <div className="space-y-1">
              <div className="text-xs text-gray-500 uppercase font-bold tracking-widest">Local Data Path</div>
              <div className="text-[10px] font-mono text-gray-400">~/Library/Application Support/PersonalAssistant/data</div>
            </div>
            <button className="p-2 hover:bg-[#262626] rounded-md text-gray-400"><ExternalLink size={16} /></button>
          </div>
        </section>

        <div className="pt-8 flex justify-end">
          <button 
            onClick={() => navTo('empty')}
            className="flex items-center gap-2 px-6 py-2 bg-white text-black font-bold rounded-lg hover:bg-gray-200 transition-colors"
          >
            <Save size={16} /> Save Changes
          </button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="flex h-screen w-full bg-[#0d0d0d] text-white font-sans overflow-hidden border border-[#333] rounded-xl">
      <Sidebar />
      <main className="flex-1 flex flex-col overflow-hidden">
        {currentScreen === 'empty' && <EmptyState />}
        {currentScreen === 'setup' && <SessionSetup />}
        {currentScreen === 'live' && <LiveSession />}
        {currentScreen === 'analysis' && <PostAnalysis />}
        {currentScreen === 'history' && <PastSession />}
        {currentScreen === 'settings' && <SettingsScreen />}
      </main>
    </div>
  );
};

export default App;
