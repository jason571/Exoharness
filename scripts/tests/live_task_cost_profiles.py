#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.
"""Opt-in real model/OCI task-cost profiling; private evidence, no media generation.

Existing report rows are never resubmitted. After interruption inspect the saved
session/run and Trace before starting another acceptance run in a new directory.
"""
import argparse
import json
import os
from pathlib import Path
import time
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--allow-model-requests', action='store_true')
parser.add_argument('--url', required=True, help='Management API origin')
parser.add_argument('--token-env', default='EXOHARNESS_TEST_ADMIN_TOKEN')
parser.add_argument('--output', required=True, help='Private evidence directory')
parser.add_argument('--case', action='append', choices=['ppt-normal','web-normal','ppt-recovery','web-recovery','web-raw'])
args = parser.parse_args()
if not args.allow_model_requests:
    parser.error('Real model cost requires --allow-model-requests')
Path(args.output).mkdir(parents=True, mode=0o700, exist_ok=True)
Path(args.output).chmod(0o700)
token = os.environ[args.token_env]
def api(path, data=None):
    request = urllib.request.Request(args.url.rstrip('/') + path,
        data=None if data is None else json.dumps(data).encode(),
        headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=70) as response:
        return json.load(response)

import zipfile,hashlib,io,xml.etree.ElementTree as ET
cases=[
 ('ppt-normal','生成一份可编辑的3页PPT，给一年级小朋友解释什么是人工智能。第一页是什么，第二页日常例子，第三页安全使用建议。字大、句短、带简单的本地矢量图形。不要调用图片、音频或视频生成服务，不使用网络资源。必须交付实际的PPTX文件。'),
 ('web-normal','生成一个介绍ExoHarness的单页网页，含产品介绍、三张功能卡、一个可点击切换的中文/英文按钮，适配手机。使用本地HTML、CSS和JavaScript及内联SVG，不引用网络资源，不调用图片、音频或视频生成。交付可实际打开的网页项目和ZIP。'),
 ('ppt-recovery','受控故障验收：先创建任务工作区，写main.py为raise RuntimeError("intentional PPT cost validation")，output_files=[]，必须先真实执行一次得到这个预期错误。然后在同一个工作区改正脚本，使用已有python-pptx生成可编辑的3页PPT，给一年级小朋友解释什么是人工智能，字大、句短、简单形状示意。不要调用图片、视频或音频生成，不联网、不安装包。最终发布实际PPTX文件。'),
 ('web-recovery','受控故障验收：先创建任务工作区，写main.py为raise RuntimeError("intentional web cost validation")，output_files=[]，必须先真实执行一次得到这个预期错误。然后改正同一工作区，生成一个介绍ExoHarness的本地单页网页，含介绍、三张功能卡、中文/英文切换按钮和手机布局。HTML、CSS、JavaScript及内联SVG，不用网络资源，不调用图片、视频或音频生成。最后发布网页项目及ZIP。')]
# Cost-oriented defaults remain unchanged. Select this case explicitly to
# exercise discovery using the exact original prompt, without tool hints.
if args.case and 'web-raw' in args.case:
 cases.insert(0, ('web-raw','为ExoHarness生成宣传网站页面'))
report_file=Path(args.output)/'report.json'
rows=json.loads(report_file.read_text()) if report_file.exists() else []
for name,prompt in cases:
 if any(row['case']==name for row in rows):continue
 if args.case and name not in args.case:continue
 if name=='web-normal':
  prompt='生成介绍Rust通用Agent框架ExoHarness的一个简洁单页网页，含产品介绍、三张功能卡、中文/英文切换按钮和手机布局。一个index.html内联CSS、JS、SVG即可，无外网和媒体服务。用已有工程沙盒workspace_create、workspace_edit保存index.html和main.py，main.py用Python标准库打包website.zip，再用sandbox_run执行并发布ZIP。预算验收只需一次基本检查，无需反复读取、美化或修订已保存的页面。'
 if name!='web-raw':
  prompt += ' 成本验收要求：workspace_id必须使用workspace_create返回的实际ID，expected_revision使用最新回执版本。直接用workspace_edit保存完整文件，不用coding_apply_patch反复美化；写齐文件后检查、打包并发布。'
 if name.endswith('recovery'):
  prompt += ' 预期故障只执行一次，随后必须先用workspace_edit改写main.py再运行，不得只改变版本号或输出文件名就重跑。PPT请直接调用已有python-pptx库，不要调用Skill自带render_deck或猜测其palette配置。'
 if name=='ppt-recovery':
  prompt += ' 本验收不用任何Skill或Skill说明，直接使用工程沙盒及已有python-pptx；实际生成后不要再调用Skill验证器，验收脚本会检查PPTX。'
 if name.startswith('web') and name!='web-raw':
  prompt += ' ExoHarness是Rust实现的通用Agent执行框架，产品文字围绕模型接入、能力管理、沙盒和文件交付。'
 s=api('/v1/sessions',{'title':'Cost acceptance '+name});sid=s['id']
 row={'case':name,'session_id':sid,'status':'submission_intent'};rows.append(row)
 report_file.write_text(json.dumps(rows,indent=2));report_file.chmod(0o600)
 result=api('/v1/sessions/'+sid+'/turns',{'prompt':prompt});rid=result['run_id']
 row.update(run_id=rid,status='submitted')
 report_file.write_text(json.dumps(rows,indent=2));report_file.chmod(0o600)
 print(json.dumps(row),flush=True)
 for _ in range(300):
  first=api('/v1/trace/runs/'+rid+'?event_limit=500');status=first['run']['status']
  if status not in ('running','queued','pending','waiting_authorization'):break
  time.sleep(2)
 else:raise RuntimeError('Existing profiling run remains active; inspect its saved ID')
 events=list(first['events']);cursor=events[-1]['sequence'] if events else 0
 while cursor<first['run']['last_sequence']:
  more=api('/v1/trace/runs/'+rid+'?event_after='+str(cursor)+'&event_limit=500')['events']
  if not more:break
  events.extend(more);cursor=more[-1]['sequence']
 private=Path(args.output)/(name+'-trace.json');private.write_text(json.dumps({**first,'events':events}));private.chmod(0o600)
 history=api('/v1/sessions/'+sid)
 row.update(status=status,model_requests=len(first['model_calls']),capability_calls=len(first['invocations']),usage=history['turns'][0].get('response',{}).get('usage'),stop_reason=next((e['payload'].get('reason') for e in reversed(events) if e['payload'].get('type')=='execution_exception' and e['payload'].get('source')=='engine'), None))
 budget=next((e['payload'] for e in events if e.get('payload',{}).get('type')=='execution_recovery_budget'),None)
 row['recovery_budget']=budget
 row['delivered_files']=[]
 cards=history['turns'][0].get('response',{}).get('artifacts',[])
 for card in cards:
  if card.get('kind') not in ('file','image'):continue
  with urllib.request.urlopen(card['download_url'],timeout=60) as response:raw=response.read()
  delivered={'filename':card['filename'],'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'download_http':200}
  if card['filename'].endswith('.pptx'):
   with zipfile.ZipFile(io.BytesIO(raw)) as package:
    slide_files=[p for p in package.namelist() if p.startswith('ppt/slides/slide') and p.endswith('.xml')]
    delivered['slides']=len(slide_files);assert len(slide_files)==3
    text=' '.join(''.join(ET.fromstring(package.read(p)).itertext()) for p in slide_files)
    delivered['has_ai_topic']='人工智能' in text or 'AI' in text;assert delivered['has_ai_topic']
  if card['filename'].endswith('.zip'):
   with zipfile.ZipFile(io.BytesIO(raw)) as package:
    delivered['zip_files']=package.namelist();assert any(p.endswith('.html') for p in package.namelist())
  if card['filename'].endswith('.html'):
   delivered['html_valid']=b'<html' in raw.lower() or b'<!doctype' in raw.lower();assert delivered['html_valid']
   if name=='web-raw':
    from html.parser import HTMLParser
    class VisibleText(HTMLParser):
     def __init__(self):super().__init__();self.hidden=0;self.text=[]
     def handle_starttag(self,tag,attrs):
      if tag in ('script','style'):self.hidden+=1
     def handle_endtag(self,tag):
      if tag in ('script','style') and self.hidden:self.hidden-=1
     def handle_data(self,data):
      if not self.hidden:self.text.append(data)
    parsed=VisibleText();parsed.feed(raw.decode('utf-8'));visible=' '.join(parsed.text)
    delivered['subject_verified']='ExoHarness' in visible and 'rust' in visible.lower() and any(term in visible.lower() for term in ('agent','智能体','执行框架')) and not any(term in visible for term in ('BOXLINK','空间娱乐操作系统'))
    assert delivered['subject_verified'],'Run success is not subject/content acceptance'
  row['delivered_files'].append(delivered)
 if name=='web-raw':
  assert status=='succeeded' and any(item['filename'].endswith('.html') for item in row['delivered_files']), 'Original prompt did not deliver an HTML page; inspect clarification or failure in the saved Trace'
 report_file.write_text(json.dumps(rows,indent=2));report_file.chmod(0o600)
 print(json.dumps(row,ensure_ascii=False),flush=True)
