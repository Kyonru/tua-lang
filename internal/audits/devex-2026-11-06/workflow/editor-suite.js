"use strict";
const fs=require('node:fs'),path=require('node:path');
exports.run=async()=>{
 const vscode=require('vscode');const {repo}=require('../harness');
 const root=process.env.TUA_AUDIT_PROJECT,output=process.env.TUA_AUDIT_OUTPUT;
 const evidence={editorVersion:vscode.version,extensionVersion:vscode.extensions.getExtension('SolenodonteLabs.tua').packageJSON.version,events:[]};
 const pause=ms=>new Promise(r=>setTimeout(r,ms));
 const open=async name=>{const d=await vscode.workspace.openTextDocument(path.join(root,'src',name));const e=await vscode.window.showTextDocument(d,{preview:false});return {d,e};};
 const atEnd=(d,needle)=>d.positionAt(d.getText().lastIndexOf(needle)+needle.length);
 await vscode.extensions.getExtension('SolenodonteLabs.tua').activate();
 let {d,e}=await open('control.tua');
 for(let i=0;i<80;i++){const h=await vscode.commands.executeCommand('vscode.executeHoverProvider',d.uri,atEnd(d,'obj.heal'));if(h?.length)break;await pause(100);}
 let t=performance.now();const control=await vscode.commands.executeCommand('vscode.executeCompletionItemProvider',d.uri,atEnd(d,'obj.'));
 evidence.events.push({id:'control-member-completion',ms:performance.now()-t,labels:control?.items.map(i=>i.label)});
 ({d,e}=await open('main.tua'));e.selection=new vscode.Selection(atEnd(d,'createEnt'),atEnd(d,'createEnt'));
 for(const mode of ['initial','after-idle']){if(mode==='after-idle')await pause(500);t=performance.now();const c=await vscode.commands.executeCommand('vscode.executeCompletionItemProvider',d.uri,e.selection.active);evidence.events.push({id:'autoimport-'+mode,ms:performance.now()-t,labels:c?.items.map(i=>i.label),text:d.getText(),position:e.selection.active});}
 const {BrowserSession}=require(path.join(repo,'editors/vscode/test/webview/cdp'));
 let browser;
 try{
  const targets=await(await fetch(`http://127.0.0.1:${process.env.TUA_AUDIT_PORT}/json/list`)).json();
  browser=await BrowserSession.connect(targets.find(t=>t.type==='page').webSocketDebuggerUrl);
  await vscode.commands.executeCommand('editor.action.triggerSuggest');await pause(500);
  const menu=await browser.evaluate("[...document.querySelectorAll('.suggest-widget')].map(n=>({text:n.innerText,visible:!!n.offsetParent}))");evidence.events.push({id:'actual-suggest-widget',menu});
  const shot=await browser.send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(output,'editor-suggest.png'),Buffer.from(shot.data,'base64'));
 }catch(error){evidence.events.push({id:'suggest-ui-blocker',error:String(error)});}finally{browser?.dispose();await vscode.commands.executeCommand('hideSuggestWidget');}
 ({d,e}=await open('unicode.tua'));const before=d.getText(),p=d.positionAt(before.lastIndexOf('score')+1);
 const hover=await vscode.commands.executeCommand('vscode.executeHoverProvider',d.uri,p);
 const edits=await vscode.commands.executeCommand('vscode.executeDocumentRenameProvider',d.uri,p,'points');
 const editRecords=edits?.entries().map(([uri,values])=>({uri:uri.toString(),edits:values}));
 const applied=edits?await vscode.workspace.applyEdit(edits):false;
 evidence.events.push({id:'unicode-rename-applied-in-editor',before,position:p,hover,edits:editRecords,applied,after:d.getText()});
 fs.writeFileSync(path.join(output,'editor-evidence.json'),JSON.stringify(evidence,null,2)+'\n');
 console.log('AUDIT_EDITOR_EVIDENCE '+JSON.stringify(evidence));
};
