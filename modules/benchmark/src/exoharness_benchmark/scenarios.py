# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Benchmark task/rule combinations; target checks are not execution evidence."""
import hashlib
from pathlib import Path
import shutil
import wave

from .contracts import Suite
from .evidence import write_private


def text_smoke():
    tasks = []
    examples = [(35, 5, 8, 42, 490), (42, 2, 6, 68, 714), (53, 3, 7, 64, 848)]
    for index, (number, old, new, delta, product) in enumerate(examples):
        tasks.append({'id': f'train_math_{index}', 'group': 'math', 'split': 'train',
            'turns': [f'两位数乘{number}，把个位{old}看成{new}，积多{delta}。只返回JSON，字段product为正确的积，不创建文件、不调用工具。'],
            'checks': [{'kind': 'json_equals', 'fields': {'product': product}}]})
    tasks.append({'id': 'test_math_composition', 'group': 'math', 'split': 'test',
        'turns': ['琪琪计算一个两位数乘24，把个位4看成7，结果比正确的积多39。求正确的积。只返回JSON，product为整数；不要创建文件或调用工具。'],
        'checks': [{'kind': 'json_equals', 'fields': {'product': 312}},
                   {'kind': 'tool_count', 'name_contains': 'generate', 'max': 0}]})
    return Suite.parse({'version': 1, 'id': 'math_composition_smoke_v1', 'tasks': tasks,
                        'purpose': 'Protocol and retention smoke only; not a generalization or media-quality claim'})


def workflows(audio_sha):
    tasks = []
    source = 'https://github.com/jason571/Exoharness'
    descriptions = {
        'video': ('生成15秒ExoHarness科技风宣传视频，最后清楚显示开源地址' + source + '，交付MP4。',
                  [{'kind': 'artifact', 'extension': '.mp4', 'min_duration': 13.5, 'max_duration': 16.5}]),
        'mv': ('使用上传的现成音频生成仙侠MV，保持原音轨，不生成音乐或歌词，交付MP4。',
               [{'kind': 'artifact', 'extension': '.mp4', 'min_duration': 1, 'audio': True},
                {'kind': 'tool_count', 'name_contains': 'generate_music', 'max': 0}]),
        'ppt': ('给一年级小朋友解释人工智能，生成3页可编辑PPT，字大句短，交付PPTX。',
                [{'kind': 'artifact', 'extension': '.pptx', 'slides': 3, 'contains': ['人工智能']}]),
        'web': ('为Rust通用Agent框架ExoHarness生成宣传网页，含手机布局、中英文切换与GitHub入口' + source + '。交付项目ZIP与预览链接。',
                [{'kind': 'artifact', 'extension': '.zip', 'contains': ['ExoHarness', 'Rust']}]),
        'music': ('使用这些明确提供的原创歌词生成古风歌曲，不自动写词，交付可播放音频：[Verse]\n山间清风过，云外月如歌。\n[Chorus]\n一盏星河落，与你看山河。',
                  [{'kind': 'artifact', 'extension': '.mp3', 'min_duration': 1, 'audio': True}]),
        'lesson': ('生成四年级数学讲解视频：一个两位数乘24，把个位4看成7，积多39，正确的积为312。请解释原因并交付视频。',
                   [{'kind': 'artifact', 'extension': '.mp4', 'min_duration': 1, 'audio': True}]),
    }
    rules = {
        'video': [('portrait', '画幅为竖屏9:16。'), ('static_outro', '片尾使用静态图片承载地址文字。'),
                  ('chinese_title', '开头显示中文标题“让智能体真正交付”。'), ('silent', '全片不包含声音。')],
        'mv': [('portrait', '画幅为竖屏9:16。'), ('mountains', '包含雾中群山场景。'),
               ('white_robes', '主角穿白衣，衣袖随风。'), ('end_title', '最后出现中文标题“云外月如歌”。')],
        'ppt': [('examples', '包含家庭和学校两种日常例子。'), ('safety', '明确说明不要分享密码。'),
                ('simple_shapes', '插图使用简单矢量形状，不生成图片。'), ('notes', '每页带一段老师讲解用的演讲备注。')],
        'web': [('offline', '所有CSS、JS、SVG内联，离线可用。'), ('keyboard', '语言切换可用键盘操作，有明确焦点。'),
                ('faq', '提供可展开的FAQ区域。'), ('mobile_menu', '手机端提供可点击的折叠菜单。')],
        'music': [('female_voice', '使用女声演唱。'), ('slow', '歌曲节奏舒缓。'),
                  ('guzheng', '突出古筝伴奏。'), ('flute', '突出笛子伴奏。')],
        'lesson': [('equation', '清楚展示27减24等于3。'), ('verify', '最后展示13乘24等于312并验算差额。'),
                   ('voice', '使用清晰普通话旁白。'), ('large_text', '公式字体大，避免密集文字。')],
    }
    variants = {'train': [(0,), (1,), (2,), (3,), (0, 1)],
                'test': [(0, 2), (1, 3), (0, 3), (1, 2), (0, 1, 2, 3)]}
    for group, (prompt, checks) in descriptions.items():
        for split, subsets in variants.items():
            for index, subset in enumerate(subsets):
                selected = [rules[group][i] for i in subset]
                task = {'id': f'{split}_{group}_{index}', 'group': group, 'split': split,
                        'turns': [prompt + ''.join(text for _, text in selected)],
                        'checks': checks, 'rule_ids': [key for key, _ in selected], 'review_required': True}
                if group == 'mv':
                    task['attachments'] = [{'type': 'audio', 'path': 'inputs/existing.wav', 'sha256': audio_sha}]
                tasks.append(task)
    # Explicit multi-Turn continuation is part of the input, never a hidden retry.
    tasks.append({'id': 'regression_lesson_continue', 'group': 'lesson', 'split': 'regression',
        'turns': [descriptions['lesson'][0], '继续完成当前任务，复用已生成音频。'],
        'checks': descriptions['lesson'][1], 'review_required': True})
    tasks.append({'id': 'regression_video_repo_endcard', 'group': 'video', 'split': 'regression',
        'turns': ['为开源的ExoHarness生成15S宣传视频，开源地址，https://github.com/jason571/Exoharness\n视频最后必须出现地址链接'],
        'checks': descriptions['video'][1], 'review_required': True})
    return Suite.parse({'version': 1, 'id': 'workflow_combinations_v1', 'tasks': tasks,
        'rules': {g: [key for key, _ in values] for g, values in rules.items()},
        'review_scope': 'Structural checks only. Content, real links, original audio, visual quality and non-regression need independent review before retention.'})


def current_reports(retrieval_tool_pattern='mcp_tavily_'):
    """Opt-in current-information delivery against a configured retrieval service."""
    tasks = [
        {'id': 'train_current_report', 'split': 'train',
         'turns': ['做一份家用储能行业现状的PPT'], 'subject': ['储能']},
        {'id': 'regression_china_economy', 'split': 'regression',
         'turns': ['写一个当前中国经济现状的PPT'], 'subject': ['中国', '经济']},
        {'id': 'test_current_vehicle_market', 'split': 'test',
         'turns': ['做一份新能源汽车市场现状的PPT'], 'subject': ['新能源', '汽车']},
    ]
    for task in tasks:
        task.update(group='current_report', review_required=True,
                    checks=[{'kind': 'artifact', 'extension': '.pptx',
                             'contains': task.pop('subject')},
                            {'kind':'tool_count','name_contains':retrieval_tool_pattern,
                             'min':1,'max':8,'successful_only':True}])
    return Suite.parse({'version': 1, 'id': 'current_reports_v2', 'tasks': tasks,
        'review_scope': 'Require real PPTX delivery, dated confirmed sources and Office rendering. No exact slide count. The China prompt is a known production regression, not an unseen test. Inspect source accuracy and actual Token use separately.'})


def create(directory, kind, mv_audio=None):
    root = Path(directory)
    if root.exists() and any(root.iterdir()):
        raise ValueError('Create a suite in a new empty directory')
    if kind == 'workflows':
        path = Path(mv_audio) if mv_audio else None
        if path is None or not path.is_file() or path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError('Workflow suite requires existing bounded WAV audio')
        try:
            with wave.open(str(path), 'rb') as audio:
                if not audio.getnframes() or not audio.getframerate() or audio.getnchannels() not in (1, 2):
                    raise ValueError('Provide a nonempty mono/stereo WAV input')
        except (wave.Error, EOFError) as error:
            raise ValueError('Provide a readable PCM WAV; no automatic generation or conversion') from error
        suite = workflows(hashlib.sha256(path.read_bytes()).hexdigest())
        (root / 'inputs').mkdir(parents=True, mode=0o700)
        shutil.copyfile(path, root / 'inputs/existing.wav')
        (root / 'inputs/existing.wav').chmod(0o600)
    elif kind == 'text-smoke':
        suite = text_smoke()
    elif kind == 'current-reports':
        suite = current_reports()
    else:
        raise ValueError('Unknown suite kind')
    write_private(root / 'suite.json', suite.value)
    return suite
