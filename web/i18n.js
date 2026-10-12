// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

"use strict";
(() => {
  // src/i18n-catalog.ts
  var entries = `
\u80FD\u529B\u53D7\u9650|Capability limited
\u6B63\u5728\u5904\u7406\u4EFB\u52A1\uFF0C\u5DF2\u5B8C\u6210\u5185\u5BB9\u4F1A\u4FDD\u7559\u3002|Processing the task. Completed work is retained.
\u76EE\u524D\u6CA1\u6709\u83B7\u5F97\u5B8C\u6210\u8FD9\u9879\u4EFB\u52A1\u6240\u9700\u7684\u6280\u80FD\u6743\u9650\uFF0C\u8BF7\u8054\u7CFB\u7BA1\u7406\u5458\u5F00\u653E\u76F8\u5E94\u6280\u80FD\u3002\u5DF2\u5B8C\u6210\u5185\u5BB9\u5DF2\u4FDD\u7559\u3002|The required Skill permission is unavailable. Please ask your administrator to enable it. Completed work is retained.
\u6240\u9700\u80FD\u529B\u6682\u65F6\u4E0D\u53EF\u7528\uFF0C\u6682\u65F6\u65E0\u6CD5\u5B8C\u6210\u8FD9\u9879\u4EFB\u52A1\u3002\u5DF2\u5B8C\u6210\u5185\u5BB9\u5DF2\u4FDD\u7559\u3002|The required capability is temporarily unavailable. Completed work is retained.
\u5DF2\u6709\u4EFB\u52A1\u7684\u6700\u7EC8\u7ED3\u679C\u6682\u65F6\u65E0\u6CD5\u786E\u8BA4\u3002\u5DF2\u5B8C\u6210\u5185\u5BB9\u5DF2\u4FDD\u7559\uFF0C\u65E0\u9700\u91CD\u590D\u63D0\u4EA4\u3002|The existing task's final result is not yet confirmed. Completed work is retained; no resubmission is needed.
\u5F53\u524D\u80FD\u529B\u6682\u65F6\u65E0\u6CD5\u5904\u7406\u5B8C\u6574\u4EFB\u52A1\uFF0C\u8BF7\u7F29\u5C0F\u672A\u5B8C\u6210\u90E8\u5206\u7684\u8303\u56F4\u3002\u5DF2\u5B8C\u6210\u5185\u5BB9\u5DF2\u4FDD\u7559\u3002|The current capability cannot handle the full task. Please reduce the remaining scope. Completed work is retained.
\u76EE\u524D\u53EF\u7528\u7684\u80FD\u529B\u6682\u65F6\u65E0\u6CD5\u5B8C\u6210\u8FD9\u9879\u4EFB\u52A1\uFF0C\u5DF2\u5B8C\u6210\u5185\u5BB9\u5DF2\u4FDD\u7559\u3002\u53EF\u8054\u7CFB\u7BA1\u7406\u5458\u68C0\u67E5\u6240\u9700\u6280\u80FD\u4E0E\u6743\u9650\u3002|The available capabilities cannot currently complete this task. Completed work is retained. Your administrator can check the required Skills and permissions.
\u8FD0\u884C\u603B\u89C8|Overview
\u6307\u5B9A\u670D\u52A1|Selected services
\uFF1A\u8FDE\u63A5 HTTP MCP|: Connect HTTP MCP
\uFF1A\u8BA4\u8BC1\u503C\u4F1A\u81EA\u52A8\u4FDD\u5B58\u5230\u672C\u673A\u5BC6\u94A5\u4FDD\u9669\u7BB1|: Credentials are saved automatically in the local secret store
\u65B0\u5EFA\u5206\u7C7B|New category
\u6DFB\u52A0\u65B0\u6A21\u578B|Add model
\u4EA4\u4ED8|Deliver
\u6267\u884C\u4E0E\u4EA4\u4ED8|Execution & delivery
\u8FDB\u884C\u4E2D|In progress
\u7B49\u5F85\u6062\u590D|Awaiting resume
\u6267\u884C\u5931\u8D25|Execution failed
\u67E5\u770B\u5BF9\u8BDD|View conversation
\u67E5\u770B\u6587\u4EF6|View file
\u53D1\u9001\u6D88\u606F\u540E\uFF0C\u53EF\u5728\u8FD9\u91CC\u67E5\u770B\u6267\u884C\u72B6\u6001\u3001\u9884\u89C8\u548C\u4E0B\u8F7D\u6587\u4EF6\u3002|After sending a message, view task status, preview results and download files here.
\u8FD9\u91CC\u663E\u793A\u5DF2\u52A0\u8F7D\u5386\u53F2\u4E2D\u6700\u8FD1 50 \u6B21\u6267\u884C\uFF1B\u66F4\u65E9\u7684\u6587\u4EF6\u53EF\u5728\u5BF9\u5E94\u5386\u53F2\u6D88\u606F\u4E2D\u67E5\u770B\u3002|The latest 50 runs in loaded history are shown here. Find earlier files in their original messages.
\u70B9\u51FB\u63D2\u5165\u53EF\u7528\u5185\u5BB9|Select to insert available content
\u6280\u80FD\u4E2D\u5FC3|Capabilities
\u6280\u80FD\u5BFC\u5165|Import capabilities
\u6A21\u578B\u914D\u7F6E|Models
\u667A\u80FD\u52A9\u7406|Assistants
\u8D44\u6599\u7BA1\u7406|Knowledge
\u8FD0\u884C\u8FFD\u8E2A|Run trace
\u64CD\u4F5C\u8BB0\u5F55|Audit log
\u7CFB\u7EDF\u8BBE\u7F6E|System settings
\u7528\u6237\u7BA1\u7406|Users
\u7BA1\u7406\u5458|Administrator
\u7BA1\u7406\u5458\u89C6\u56FE|Administrator view
\u7BA1\u7406\u63A7\u5236\u53F0|Management console
\u53EA\u8BFB\u7528\u6237|Read-only user
\u9000\u51FA|Sign out
\u9000\u51FA\u767B\u5F55|Sign out
\u767B\u5F55|Sign in
\u767B\u5F55 ExoHarness|Sign in to ExoHarness
\u6B22\u8FCE\u56DE\u6765|Welcome back
\u767B\u5F55\u540E\u7EE7\u7EED\u4F60\u7684\u4E13\u5C5E\u5BF9\u8BDD\u3002|Sign in to continue your conversations.
\u8D26\u53F7|Account
\u90AE\u7BB1|Email
\u90AE\u7BB1\u6216\u7528\u6237\u540D|Email or username
\u5BC6\u7801|Password
\u663E\u793A\u5BC6\u7801|Show password
\u9690\u85CF\u5BC6\u7801|Hide password
\u663E\u793A\u786E\u8BA4\u5BC6\u7801|Show confirmation password
\u9690\u85CF\u786E\u8BA4\u5BC6\u7801|Hide confirmation password
\u786E\u8BA4\u5BC6\u7801|Confirm password
\u521B\u5EFA\u8D26\u53F7|Create account
\u5DF2\u6709\u8D26\u53F7\uFF1F|Already have an account?
\u8FD8\u6CA1\u6709\u8D26\u53F7\uFF1F|New to ExoHarness?
\u4F7F\u7528\u90AE\u7BB1\u6CE8\u518C|Register with email
\u7528\u6237\u540D|Username
\u90AE\u7BB1\u9A8C\u8BC1\u7801|Email verification code
\u53D1\u9001\u9A8C\u8BC1\u7801|Send code
\u8FD4\u56DE\u767B\u5F55|Back to sign in
\u9A8C\u8BC1|Verify
6 \u4F4D\u6570\u5B57|6 digits
\u9A8C\u8BC1\u7801\u5C06\u53D1\u9001\u5230\u4F60\u7684\u90AE\u7BB1\uFF0C\u9A8C\u8BC1\u5B8C\u6210\u540E\u624D\u80FD\u6CE8\u518C\u3002|A verification code will be sent to your email. Verify it to register.
\u521B\u5EFA\u8D26\u53F7\uFF0C\u5F00\u542F\u5C5E\u4E8E\u4F60\u7684\u521B\u4F5C\u7A7A\u95F4\u3002\u60F3\u6CD5\u3001\u7D20\u6750\u4E0E\u4F5C\u54C1\uFF0C\u90FD\u5728\u540C\u4E00\u4E2A\u5730\u65B9\u5EF6\u7EED\u3002|Create an account to keep your ideas, materials and creations in one place.
\u4ECE\u4ECA\u5929\u5F00\u59CB\uFF0C|Start today.
\u628A\u7075\u611F\u5B9E\u73B0\u3002|Bring your ideas to life.
\u8BA9\u6BCF\u4E00\u4E2A\u60F3\u6CD5\uFF0C|Give every idea
\u90FD\u6709\u56DE\u54CD\u3002|a voice.
\u770B\u89C1\u6BCF\u4E00\u6B21\u6267\u884C\uFF0C|Follow every task.
\u638C\u63A7\u6BCF\u4E00\u9879\u80FD\u529B\u3002|Manage every capability.
\u7BA1\u7406\u6A21\u578B\u3001\u80FD\u529B\u4E0E\u8FD0\u884C\u72B6\u6001\uFF0C\u8BA9\u667A\u80FD\u4F53\u5728\u6E05\u6670\u7684\u8FB9\u754C\u5185\u53EF\u9760\u5DE5\u4F5C\u3002|Manage models, capabilities and execution within defined permissions.
\u7BA1\u7406\u5458\u53EF\u4FEE\u6539\u7CFB\u7EDF\u914D\u7F6E\uFF1B\u53EA\u8BFB\u7528\u6237\u53EF\u67E5\u770B\u83B7\u6388\u6743\u7684\u7BA1\u7406\u9875\u9762\u3002\u6B63\u5F0F\u804A\u5929\u8BF7\u4F7F\u7528\u7528\u6237\u7AEF\u3002|Administrators manage configuration. Read-only users can view authorized pages. Use the user portal for chat.
\u8F93\u5165\u7BA1\u7406\u5458\u5BC6\u7801\u6216\u53EA\u8BFB\u7528\u6237\u5BC6\u7801\uFF0C\u7CFB\u7EDF\u4F1A\u81EA\u52A8\u8FDB\u5165\u5BF9\u5E94\u6743\u9650\u3002|Enter the administrator or read-only password to access your assigned role.
\u5F53\u524D\u7BA1\u7406\u5BC6\u7801|Current administrator password
\u767B\u5F55\u5BC6\u7801|Sign-in password
\u4E3B\u5BFC\u822A|Main navigation
\u5C55\u5F00\u83DC\u5355|Expand menu
\u6536\u8D77\u83DC\u5355|Collapse menu
\u6253\u5F00\u7BA1\u7406\u83DC\u5355|Open management menu
\u5173\u95ED\u7BA1\u7406\u83DC\u5355|Close management menu
\u6267\u884C\u8FDB\u5EA6|Execution progress
\u6B63\u5728\u540C\u6B65 Registry|Syncing registry
\u6B63\u5728\u540C\u6B65\u4F1A\u8BDD\u8BB0\u5F55|Syncing conversations
\u6682\u65E0\u4F1A\u8BDD\u8BB0\u5F55|No conversations yet
\u6B63\u5728\u540C\u6B65\u64CD\u4F5C\u8BB0\u5F55|Syncing audit records
\u90AE\u4EF6\u670D\u52A1\u5DF2\u8FDE\u63A5|Mail service connected
\u8F93\u5165\u7ED3\u6784|Input structure
\u6A21\u578B\u8F93\u5165\u7ED3\u6784\uFF08\u4E0D\u4FDD\u5B58\u539F\u6587\uFF09|Model input structure (original not stored here)
\u672A\u8BB0\u5F55\uFF08\u4E0D\u7B49\u4E8E\u5B9E\u9645\u8BA1\u8D39\u8282\u7701\uFF09|Not recorded (not equivalent to billing savings)
\u6A21\u578B\u8F93\u5165\u8BC1\u636E|Model input evidence
\u6A21\u578B\u914D\u7F6E|Model configuration
\u7CFB\u7EDF\u8BF4\u660E|System instructions
\u5DE5\u5177 Schema|Tool schema
\u5DF2\u8131\u654F|Redacted
\u5A92\u4F53\u8F6C\u4E3A\u5F15\u7528|Media references
\u6280\u80FD\u8D44\u6E90\u53BB\u91CD|Deduplicated Skill resources
\u79FB\u9664\u672A\u6267\u884C\u7684\u9519\u8BEF\u53C2\u6570|Removed rejected arguments
\u5C40\u90E8\u4FEE\u590D\u4E0A\u4E0B\u6587|Local repair context
\u4FDD\u7559\u9636\u6BB5\u7ED3\u679C\u5F15\u7528|Retained checkpoint references
\u79FB\u51FA\u672A\u9009\u7528\u7684\u6280\u80FD|Removed unselected Skills
\u79FB\u51FA\u5DF2\u66FF\u4EE3\u7684\u68C0\u67E5\u70B9|Removed superseded checkpoints
\u9009\u62E9\u672C\u6B65\u5DE5\u5177|Selected tools for this step
\u6574\u7406\u5386\u53F2\u4E0A\u4E0B\u6587|Compacted history
\u5F53\u524D\u5DE5\u4F5C\u6D41\u4E0A\u4E0B\u6587|Workflow context
\u5F53\u524D\u53C2\u6570\u4FEE\u590D\u4E0A\u4E0B\u6587|Argument repair context
\u5927\u7ED3\u679C\u4FDD\u5B58\u4E3A\u5F15\u7528|Persisted large-result reference
\u6062\u590D\u4FDD\u5B58\u7684\u4E0A\u4E0B\u6587|Restored context
\u9009\u53D6\u5F85\u6574\u7406\u5185\u5BB9|Selected history for compaction
\u79FB\u9664 Schema \u672A\u5F15\u7528\u7684\u5B9A\u4E49\uFF08\u4FDD\u7559\u7EA6\u675F\uFF09|Removed unreachable schema definitions (constraints retained)
\u9009\u53D6\u8FDB\u5C55\u68C0\u67E5\u5185\u5BB9|Selected loop-review context
\u79FB\u51FA\u672A\u63D0\u4EA4\u7684\u56DE\u590D|Removed unsubmitted response
\u7EA0\u6B63\u672A\u5B9E\u9645\u6267\u884C\u7684\u56DE\u590D|Corrected unexecuted response
\u8FD0\u884C\u4FE1\u606F|Run information
\u7ED3\u675F\u8BE6\u60C5|Completion details
\u8FD0\u884C\u72B6\u6001\u4E0E\u9884\u7B97|Run status and budget
\u7ED3\u675F\u539F\u56E0\u4E0E\u5173\u8054\u9519\u8BEF|Completion reason and related errors
\u7528\u91CF\u5BF9\u8D26\u5F02\u5E38|Usage reconciliation mismatch
\u8FD0\u884C\u8F68\u8FF9\u5DF2\u540C\u6B65|Trace synchronized
\u6B63\u5728\u8BFB\u53D6\u8FD0\u884C\u8F68\u8FF9|Loading trace
\u8FD0\u884C\u4E2D|Running
\u8FD0\u884C\u4E2D \xB7 \u81EA\u52A8\u5237\u65B0|Running \xB7 Auto-refresh
\u672A\u63D0\u4F9B|Not provided
\u672A\u8BB0\u5F55|Not recorded
Provider \u672A\u63D0\u4F9B|Not reported by provider
\u5B9E\u9645 Token|Reported tokens
\u4E0A\u4E0B\u6587\u4F30\u7B97|Context estimate
\u8017\u65F6|Duration
\u7528\u6237\u8BF7\u6C42|User request
\u8FD0\u884C\u65F6|Runtime
\u89E3\u6790\u4E0E\u89C4\u5212|Analysis and planning
\u8BF7\u6C42|Request
\u54CD\u5E94|Response
\u53D1\u8D77\u80FD\u529B\u8C03\u7528|Invoke capability
\u5DF2\u8FD4\u56DE|Returned
\u7B49\u5F85\u8FD4\u56DE|Awaiting response
\u5904\u7406\u8C03\u7528\u7ED3\u679C|Process tool result
\u751F\u6210\u6700\u7EC8\u54CD\u5E94|Generate final response
\u76F4\u63A5\u54CD\u5E94|Direct response
\u53C2\u6570\u6821\u9A8C\u5931\u8D25 \xB7 \u672A\u6267\u884C|Argument validation failed \xB7 Not executed
\u8C03\u7528\u524D\u6821\u9A8C|Pre-execution validation
\u65E7\u8FD0\u884C\u6216\u5C1A\u672A\u53D1\u8D77\u6A21\u578B\u8BF7\u6C42\uFF0C\u6682\u65E0\u9010\u6B21 Token \u6570\u636E|No per-request token data for this run yet
\u63A5\u6536\u8BF7\u6C42|Request received
\u4E8B\u4EF6\u5DF2\u8BB0\u5F55|Event recorded
\u67E5\u770B\u539F\u59CB\u4E8B\u4EF6|View raw event
\u521B\u5EFA\u8FD0\u884C|Run created
\u8FD0\u884C\u72B6\u6001|Run status
\u6392\u961F\u4E2D|Queued
\u52A9\u624B\u5F00\u59CB\u54CD\u5E94|Assistant response started
\u52A9\u624B\u8F93\u51FA|Assistant output
\u672C\u8F6E\u54CD\u5E94\u7ED3\u675F|Turn response ended
\u51C6\u5907\u8C03\u7528|Prepare invocation
\u51C6\u5907\u8C03\u7528\uFF08\u5C1A\u672A\u63D0\u4EA4\uFF09|Invocation prepared (not submitted)
\u63D0\u4EA4\u8C03\u7528|Submit invocation
\u5DF2\u63D0\u4EA4\u8C03\u7528|Invocation submitted
\u8C03\u7528\u80FD\u529B|Invoke capability
\u8C03\u7528\u7ED3\u7B97|Invocation resolved
\u8C03\u7528\u5DF2\u7ED3\u7B97|Invocation resolved
\u67E5\u770B\u8FD4\u56DE\u7ED3\u679C|View result
\u67E5\u770B\u8C03\u7528\u53C2\u6570|View arguments
\u80FD\u529B\u8FD4\u56DE|Capability result
\u80FD\u529B\u8FD4\u56DE\u9519\u8BEF|Capability error
\u6062\u590D\u72B6\u6001|Recovery status
\u72B6\u6001\u66F4\u65B0|Status update
\u6267\u884C\u9636\u6BB5|Execution phase
\u6267\u884C\u8D44\u6E90|Execution resources
\u80FD\u529B\u9009\u62E9|Capability selection
\u6267\u884C\u9519\u8BEF|Execution error
\u4F9D\u8D56\u8FDE\u63A5|Dependency connections
\u5C55\u5F00\u5168\u6587|Expand text
\u6536\u8D77\u5168\u6587|Collapse text
\uFF08\u7A7A\u767D\u7247\u6BB5\uFF09|(Whitespace)
\u8BE5 Run \u6CA1\u6709\u6301\u4E45\u5316\u4E8B\u4EF6|No persisted events for this run
\u53C2\u6570\u6458\u8981|Argument digest
\u7C7B\u578B|Type
\u8C03\u7528\u7C7B\u578B|Call type
\u72B6\u6001 / \u505C\u6B62\u539F\u56E0|Status / Stop reason
\u9996\u4E8B\u4EF6\u65F6\u5EF6|Time to first event
\u603B\u8017\u65F6|Total duration
\u8F93\u5165 Token\uFF08\u4E0D\u542B\u7F13\u5B58\uFF09|Input tokens (excluding cache)
\u8F93\u51FA Token|Output tokens
\u7F13\u5B58 Token|Cached tokens
\u5DE5\u5177 Schema\uFF08\u4F30\u7B97\uFF09|Tool schemas (estimate)
\u7CFB\u7EDF\u63D0\u793A\uFF08\u4F30\u7B97\uFF09|System prompt (estimate)
\u7528\u6237\u6D88\u606F\uFF08\u4F30\u7B97\uFF09|User messages (estimate)
\u5386\u53F2\u52A9\u7406\u6D88\u606F\uFF08\u4F30\u7B97\uFF09|Assistant history (estimate)
\u5DE5\u5177\u7ED3\u679C\uFF08\u4F30\u7B97\uFF09|Tool results (estimate)
\u6458\u8981\u4E0E\u6267\u884C\u5F15\u7528\uFF08\u4F30\u7B97\uFF09|Summaries and execution references (estimate)
Skill \u8BF4\u660E / \u5F15\u7528\uFF08\u6458\u8981\u5185\u4F30\u7B97\uFF09|Skill instructions / references (within summaries)
Checkpoint\uFF08\u6458\u8981\u5185\u4F30\u7B97\uFF09|Checkpoints (within summaries)
\u5C40\u90E8\u4FEE\u590D\uFF08\u6458\u8981\u5185\u4F30\u7B97\uFF09|Local repairs (within summaries)
\u5927\u7ED3\u679C\u5F15\u7528\uFF08\u6458\u8981\u5185\u4F30\u7B97\uFF09|Large-result references (within summaries)
\u5DE5\u5177\u53C2\u6570\uFF08\u52A9\u7406\u6D88\u606F\u5185\u4F30\u7B97\uFF09|Tool arguments (within assistant messages)
Schema \u6295\u5F71\u51CF\u5C11\uFF08\u4F30\u7B97\uFF09|Schema projection savings (estimate)
\u6B63\u5728\u8BFB\u53D6\u8BF7\u6C42\u6E05\u5355\u2026|Loading request manifest\u2026
\u8BF7\u6C42\u6E05\u5355\u4E0E\u53D8\u5316|Request manifest and changes
\u8FD9\u6B21\u5386\u53F2\u8BF7\u6C42\u672A\u4FDD\u5B58\u8F93\u5165\u6E05\u5355\uFF0C\u4ECD\u53EF\u67E5\u770B\u539F\u6709\u7528\u91CF\u548C\u7ED3\u6784\u6307\u6807\u3002|This historical request has no input manifest; usage and structure metrics remain available.
\u8F93\u51FA\u6458\u8981|Output summary
\u6A21\u578B\u8F93\u51FA\u6458\u8981\uFF08\u4E0D\u91CD\u590D\u4FDD\u5B58\u539F\u6587\uFF09|Model output summary (original retained separately)
\u672A\u5339\u914D\u4F9D\u8D56\u5730\u5740\uFF0C\u4FDD\u7559\u670D\u52A1\u81EA\u8EAB\u914D\u7F6E|No dependency address matched; keeping the service configuration
\u5C1A\u672A\u8BBE\u7F6E|Not set
\u6D3B\u8DC3\u4EFB\u52A1|Active tasks
\u6B63\u5728\u6392\u961F|Queued
\u5931\u8D25 / \u53EF\u6062\u590D|Failed / Recoverable
\u671F\u95F4\u6267\u884C\u6B21\u6570|Runs in period
\u671F\u95F4 Token|Tokens in period
\u4EFB\u52A1|Task
\u6700\u65B0\u72B6\u6001|Latest status
\u63D0\u4EA4\u65F6\u95F4|Submitted
\u8FFD\u8E2A|Trace
\u67E5\u770B|View
\u6267\u884C\u8D44\u6E90\u5C1A\u672A\u542F\u7528|Execution resources disabled
\u6267\u884C\u6B21\u6570\uFF08\u542B\u91CD\u8BD5\uFF09|Runs (including retries)
\u6210\u529F\u5C1D\u8BD5|Successful attempts
\u5931\u8D25 / \u53EF\u6062\u590D\u5C1D\u8BD5|Failed / recoverable attempts
\u5DF2\u7ED3\u675F\u8FD0\u884C\u8017\u65F6|Completed run duration
\u6A21\u578B / \u670D\u52A1\u5546|Model / Provider
\u8BF7\u6C42\u6B21\u6570|Requests
\u7D2F\u8BA1\u8BF7\u6C42\u8017\u65F6|Total request duration
\u7528\u91CF\u7F3A\u5931|Missing usage
\u5168\u90E8\u7528\u6237|All users
\u672A\u6307\u5B9A\u52A9\u7406|Unassigned assistant
\u5F53\u524D\u4EFB\u52A1\u6309\u6700\u65B0\u6267\u884C\u8BA1\u6570\uFF0C\u7528\u91CF\u5305\u542B\u5386\u53F2\u91CD\u8BD5|Tasks count the latest run; usage includes retries
\u6267\u884C\u8D44\u6E90\u4E0E\u80FD\u529B\u68C0\u67E5\u4E3A\u5168\u5C40|Resources and capability checks are global
\u6BCF 30 \u79D2\u66F4\u65B0|Refreshes every 30 seconds
\u80FD\u529B\u8303\u56F4\u53CA\u58F0\u660E\u4F9D\u8D56\u68C0\u67E5\u901A\u8FC7|Capability scope and declared dependencies verified
\u6267\u884C\u8FDE\u63A5\u6216\u5185\u5BB9\u5C1A\u672A\u5C31\u7EEA|Execution connection or content not ready
\u67E5\u770B\u52A9\u7406|View assistant
\u6B64\u8303\u56F4\u5185\u6682\u65E0\u8BB0\u5F55|No records in this range
\u6B64\u7528\u6237\u5728\u6240\u9009\u65F6\u95F4\u8303\u56F4\u5185\u6682\u65E0\u6267\u884C\u8BB0\u5F55\u3002|No runs for this user in the selected period.
\u6240\u9009\u65F6\u95F4\u8303\u56F4\u5185\u6682\u65E0\u6267\u884C\u8BB0\u5F55\u3002|No runs in the selected period.
\u8FD0\u884C\u603B\u89C8\u6682\u672A\u5F00\u653E\uFF0C\u8BF7\u7A0D\u540E\u518D\u8BD5\u3002|Overview unavailable. Try again later.
\u5F53\u524D\u8D26\u53F7\u65E0\u6743\u67E5\u770B\u8FD0\u884C\u603B\u89C8\u3002|This account cannot access the overview.
\u6682\u65F6\u65E0\u6CD5\u8BFB\u53D6\u8FD0\u884C\u8BB0\u5F55\uFF0C\u8BF7\u7A0D\u540E\u91CD\u8BD5\u3002|Unable to load run records. Try again later.
\u6682\u65F6\u65E0\u6CD5\u5237\u65B0\uFF0C\u4EE5\u4E0B\u4E3A\u4E0A\u6B21\u8BFB\u53D6\u7684\u8BB0\u5F55\u3002|Unable to refresh. Showing previously loaded records.
\u8FD0\u884C\u8BB0\u5F55\u6682\u4E0D\u53EF\u7528\u3002|Run records unavailable.
\u5C1A\u672A\u767B\u8BB0\u80FD\u529B|No capabilities registered
\u6240\u6709\u5DF2\u767B\u8BB0\u80FD\u529B\u5747\u5DF2\u5C31\u7EEA|All registered capabilities are ready
\u52A9\u7406\u5DF2\u505C\u7528|Assistant disabled
\u5168\u90E8\u9879\u76EE|All projects
\u505C\u7528\u670D\u52A1|Disable service
\u542F\u7528\u670D\u52A1|Enable service
\u65E0\u9700\u989D\u5916\u6743\u9650|No additional permissions required
\u65E0\u989D\u5916\u6743\u9650\u58F0\u660E|No additional permissions declared
\u8FD9\u9879\u80FD\u529B\u4E0D\u9700\u8981\u989D\u5916\u6743\u9650\uFF0C\u53EF\u4EE5\u76F4\u63A5\u9A8C\u8BC1\u3002|This capability needs no additional permissions and can be tested directly.
\u8FD9\u9879\u80FD\u529B\u6240\u9700\u7684\u6743\u9650\u5DF2\u7ECF\u6388\u6743\u3002|The required permissions have been granted.
\u6C38\u4E45\u5220\u9664\u8FD9\u9879 Skill \u7684\u767B\u8BB0\u3001\u5168\u90E8\u5386\u53F2\u7248\u672C\u548C\u670D\u52A1\u5668\u5305\u6587\u4EF6\u3002\u6B64\u64CD\u4F5C\u65E0\u6CD5\u6062\u590D\uFF0C\u64CD\u4F5C\u8BB0\u5F55\u4ECD\u4F1A\u4FDD\u7559\u3002|Permanently delete this Skill registration, all revisions and server package files. This cannot be undone. Audit records are retained.
\u4ECE\u670D\u52A1\u5217\u8868\u79FB\u9664\u8FD9\u9879\u767B\u8BB0\uFF0C\u5E76\u5220\u9664\u5B83\u4FDD\u5B58\u5728\u672C\u673A\u7684\u8BA4\u8BC1\u4FE1\u606F\u3002\u64CD\u4F5C\u8BB0\u5F55\u4ECD\u4F1A\u4FDD\u7559\u3002|Remove this registration and its locally stored credentials. Audit records are retained.
\u5F7B\u5E95\u5220\u9664 Skill|Permanently delete Skill
API Key \u5DF2\u914D\u7F6E\uFF1B\u9875\u9762\u548C API \u5747\u4E0D\u4F1A\u56DE\u663E|API key configured; its value is never returned by the page or API
\u542F\u7528\u8FD9\u4E2A LLM\uFF1B\u4FDD\u5B58\u540E\u81EA\u52A8\u505C\u7528\u540C\u7C7B\u578B\u7684\u5176\u4ED6\u914D\u7F6E\uFF0C\u4E0D\u5F71\u54CD\u5176\u4ED6\u7C7B\u578B|Enable this LLM; saving disables other configurations of the same type without affecting other types
\u5DF2\u542F\u7528 \xB7 \u5BF9\u8BDD\u53EF\u7528|Enabled \xB7 Available in chat
\u5DF2\u542F\u7528 \xB7 \u8BED\u97F3\u8F93\u5165|Enabled \xB7 Voice input
\u6D4B\u8BD5 API \u8FDE\u63A5|Test API connection
\u751F\u6210\u77ED\u97F3\u9891\u6D4B\u8BD5|Test short audio synthesis
\u6D4B\u8BD5\u8F6C\u5F55\u63A5\u53E3|Test transcription API
\u7528\u4E8E\u804A\u5929\u5F55\u97F3\u8F6C\u6587\u5B57\uFF0C\u4E0D\u6D88\u8017 LLM Token|Transcribes chat recordings without using LLM tokens
\u8F93\u51FA\u957F\u5EA6\u7531\u6A21\u578B\u670D\u52A1\u51B3\u5B9A|Output length determined by the model service
\u66F4\u6362\u5934\u50CF|Change avatar
\u914D\u7F6E\u5DF2\u4FDD\u5B58|Configuration saved
\u5220\u9664\u8FD9\u4E2A\u52A9\u7406|Delete this assistant
\u8FD8\u6CA1\u6709\u8D44\u6599\u5E93\u3002\u5728\u53F3\u4FA7\u521B\u5EFA\u540E\uFF0C\u5373\u53EF\u5206\u7C7B\u5E76\u5BFC\u5165\u6587\u4EF6\u3002|No knowledge libraries yet. Create one on the right to organize and import files.
\u7CFB\u7EDF\u64CD\u4F5C|System action
\u5176\u4ED6|Other
\u66F4\u65B0\u6743\u9650|Update permissions
\u542F\u7528\u80FD\u529B|Enable capability
\u505C\u7528\u80FD\u529B|Disable capability
\u79FB\u9664\u80FD\u529B|Remove capability
\u66F4\u65B0\u80FD\u529B|Update capability
\u6DFB\u52A0\u80FD\u529B|Add capability
\u6062\u590D\u65E7\u7248\u672C|Restore previous revision
\u5B8C\u6210\u9A8C\u8BC1|Validation completed
\u5F00\u59CB\u8C03\u7528|Invocation started
\u8C03\u7528\u5B8C\u6210|Invocation completed
\u8C03\u7528\u5931\u8D25|Invocation failed
\u5DF2\u79FB\u9664\u7684\u80FD\u529B|Removed capability
\u7CFB\u7EDF|System
\u63A7\u5236\u53F0|Console
\u81EA\u52A8\u4EFB\u52A1|Automated task
\u672A\u77E5\u6765\u6E90|Unknown source
\u90AE\u4EF6\u670D\u52A1\u672A\u914D\u7F6E|Mail service not configured
\u5F53\u524D\u4F7F\u7528\u9ED8\u8BA4\u5185\u5BB9|Using default content
\u6B64\u529F\u80FD\u5C1A\u672A\u63A5\u5165\u65B0\u6846\u67B6|This feature is not available yet
\u4E2A\u4F1A\u8BDD|chats
\u5173\u95ED\u83DC\u5355|Close menu
\u7BA1\u7406\u83DC\u5355|Management menu
\u4E86\u89E3\u4EFB\u52A1\u8FDB\u5C55\u3001\u8D44\u6E90\u5360\u7528\u548C\u4F7F\u7528\u60C5\u51B5\u3002|View task progress, resource allocation and usage.
\u4EFB\u52A1\u4E0E\u72B6\u6001|Tasks and status
\u4EFB\u52A1\u6307\u6807|Task metrics
\u4F1A\u8BDD\u8BB0\u5F55|Conversations
\u4F1A\u8BDD\u5185\u5404\u8F6E\u6D88\u606F|Conversation turns
\u4F1A\u8BDD\u7D2F\u8BA1 Token|Total conversation tokens
\u4F1A\u8BDD\u8FFD\u8E2A|Conversation trace
\u4F1A\u8BDD\u603B\u6570|Conversation count
\u5BF9\u8BDD\u603B\u6570|Conversation count
\u80FD\u529B\u6982\u51B5|Capability summary
\u7528\u6237\u7EDF\u8BA1|User statistics
\u6A21\u578B\u6D88\u8017|Model usage
\u7528\u91CF\u7EDF\u8BA1|Usage
\u7EDF\u8BA1\u7EF4\u5EA6|Group by
\u7EDF\u8BA1\u8303\u56F4|Period
\u6700\u8FD1 24 \u5C0F\u65F6|Last 24 hours
\u6700\u8FD1 7 \u5929|Last 7 days
\u6700\u8FD1 30 \u5929|Last 30 days
\u6700\u8FD1 90 \u5929|Last 90 days
\u672C\u8F6E Token|Tokens this turn
\u672C\u8F6E\u8017\u65F6|Turn duration
\u8F93\u5165 Token|Input tokens
\u8F93\u51FA Token|Output tokens
\u7F13\u5B58 Token|Cached tokens
\u8D39\u7528|Cost
\u6A21\u578B\u8BF7\u6C42|Model requests
\u80FD\u529B\u8C03\u7528|Tool calls
\u8FD0\u884C\u6458\u8981|Run summary
\u6709\u6267\u884C\u8BB0\u5F55|With executions
\u5168\u90E8|All
\u5168\u90E8\u8BB0\u5F55|All records
\u5168\u90E8\u8FFD\u8E2A|All traces
\u641C\u7D22\u4F1A\u8BDD\u3001\u6700\u8FD1\u6D88\u606F\u3001\u72B6\u6001|Search conversations, recent messages or status
\u641C\u7D22\u7528\u6237|Search users
\u7528\u6237|User
\u7528\u6237\u540D\u79F0|User name
\u5237\u65B0|Refresh
\u5237\u65B0\u5217\u8868|Refresh list
\u6700\u8FD1 200 \u4E2A\u4F1A\u8BDD \xB7 \u8865\u5145\u6D88\u606F\u81EA\u52A8\u5F52\u7EC4|Latest 200 conversations \xB7 Follow-up messages grouped automatically
\u6BCF\u8F6E\u4EFB\u52A1\u663E\u793A\u6700\u65B0\u4E00\u6B21\u6267\u884C\uFF0C\u4F18\u5148\u663E\u793A\u6D3B\u8DC3\u4EFB\u52A1\uFF0C\u6700\u591A 50 \u9879\u3002\u70B9\u51FB\u67E5\u770B\u76F4\u63A5\u6253\u5F00\u8BE5\u6B21 Trace\uFF0C\u5386\u53F2\u5C1D\u8BD5\u4ECD\u4FDD\u7559\u3002|Shows the latest execution for each turn, with active tasks first, up to 50 items. Open a task to view its trace. Previous attempts remain available.
\u8D44\u6E90\u6C60\u5360\u7528\u6765\u81EA\u8C03\u5EA6\u5668\uFF0C\u4E0D\u7B49\u540C\u4E8E GPU \u663E\u5B58\u68C0\u6D4B\u3002\u5DF2\u6392\u961F\u4EFB\u52A1\u4F1A\u5728\u9650\u5236\u5185\u81EA\u52A8\u6267\u884C\u3002|Resource allocation is reported by the scheduler, not GPU memory monitoring. Queued tasks start within the configured limits.
\u6309\u5B9E\u9645\u65E5\u5FD7\u7EDF\u8BA1\uFF0C\u6700\u591A\u5C55\u793A\u7528\u91CF\u6700\u9AD8\u7684 50 \u9879\u3002\u672A\u4E0A\u62A5 Token \u7684\u6A21\u578B\u8BF7\u6C42\u5355\u72EC\u6807\u6CE8\uFF1B\u672A\u914D\u7F6E\u4EF7\u683C\u65F6\u4E0D\u4F30\u7B97\u8D39\u7528\u3002|Usage is calculated from logs for the top 50 items. Requests without token reports are marked separately. Costs require configured prices.
\u540C\u4E00\u4F1A\u8BDD\u7684\u6D88\u606F\u96C6\u4E2D\u5C55\u793A\uFF0C\u6309\u8F6E\u67E5\u770B\u6A21\u578B\u6D88\u8017\u3001\u80FD\u529B\u8C03\u7528\u4E0E\u6267\u884C\u7ED3\u679C\u3002|Messages are grouped by conversation. View model usage, tool calls and results for each turn.
\u96C6\u4E2D\u7BA1\u7406 Skill\u3001MCP \u4E0E\u672C\u5730\u5DE5\u5177\uFF0C\u67E5\u770B\u53EF\u7528\u6027\u3001\u6743\u9650\u548C\u542F\u7528\u72B6\u6001\u3002|Manage Skills, MCP services and local tools, including readiness, permissions and activation.
\u4ECE\u5DE6\u4FA7\u9009\u62E9\u540E\uFF0C\u53EF\u4EE5\u68C0\u67E5\u53EF\u7528\u6027\u3001\u5904\u7406\u6743\u9650\u5E76\u51B3\u5B9A\u662F\u5426\u542F\u7528\u3002|Select a capability to check readiness, manage permissions and enable it.
\u5148\u68C0\u67E5\uFF0C\u518D\u542F\u7528|Check before enabling
\u6DFB\u52A0\u80FD\u529B|Add capability
\u9009\u62E9\u4E00\u9879\u80FD\u529B|Select a capability
\u5168\u90E8\u80FD\u529B|All capabilities
\u80FD\u529B\u5217\u8868|Capability list
\u80FD\u529B\u7C7B\u522B|Capability category
\u80FD\u529B\u7C7B\u578B|Capability type
\u80FD\u529B\u7C7B\u578B\u7B5B\u9009|Filter capability types
\u80FD\u529B\u8303\u56F4|Capability scope
\u80FD\u529B\u7BA1\u7406|Capability management
\u80FD\u529B ID|Capability ID
\u80FD\u529B|Capability
\u80FD\u529B\u53D8\u66F4|Capability changes
\u80FD\u529B\u6388\u6743\u8303\u56F4|Capability permissions
\u80FD\u529B\u4E0E\u52A9\u7406\u68C0\u67E5|Capability and assistant checks
\u53EF\u7528\u6027|Readiness
\u5F53\u524D\u53EF\u7528|Available
\u5DF2\u542F\u7528|Enabled
\u542F\u7528|Enable
\u7981\u7528|Disable
\u5DF2\u7981\u7528|Disabled
\u9A8C\u8BC1\u53EF\u7528\u6027|Check availability
\u9A8C\u8BC1\u8BB0\u5F55|Validation history
\u5C1A\u672A\u9A8C\u8BC1|Not checked
\u5C1A\u672A\u8BBE\u7F6E|Not configured
\u5C1A\u672A\u4FEE\u6539|Unchanged
\u5220\u9664\u767B\u8BB0|Delete registration
\u5220\u9664\u80FD\u529B|Delete capability
\u6743\u9650|Permissions
\u8BA4\u8BC1\u4FE1\u606F|Credentials
\u5DE5\u5177\u8BF4\u660E|Tool description
\u6280\u80FD\u8BF4\u660E|Skill instructions
\u6280\u80FD\u5B8C\u6574\u8BF4\u660E|Full Skill instructions
\u5F53\u524D\u4E0D\u53EF\u53D8 revision \u7684 SKILL.md\uFF0C\u53EA\u8BFB\u5C55\u793A\u3002|Read-only SKILL.md for the current immutable revision.
\u590D\u5236\u8BF4\u660E|Copy instructions
\u590D\u5236 JSON|Copy JSON
\u70B9\u51FB\u201C\u9A8C\u8BC1\u53EF\u7528\u6027\u201D\uFF0C\u68C0\u67E5\u8FD9\u9879\u80FD\u529B\u5F53\u524D\u662F\u5426\u53EF\u4EE5\u6B63\u5E38\u4F7F\u7528\u3002|Select Check availability to test this capability.
\u8BA4\u8BC1\u5185\u5BB9\u53EA\u4FDD\u5B58\u5728\u672C\u673A\uFF0C\u4E0D\u4F1A\u663E\u793A\u5728\u9875\u9762\u3001\u914D\u7F6E\u8BE6\u60C5\u6216 API \u54CD\u5E94\u4E2D\u3002\u53EF\u5728\u8FD9\u91CC\u76F4\u63A5\u66F4\u65B0\u3001\u8F6E\u6362\u6216\u5220\u9664\u3002|Credentials are stored locally and omitted from pages, configuration details and API responses. Update, rotate or delete them here.
\u4ECE\u670D\u52A1\u5217\u8868\u79FB\u9664\u8FD9\u9879\u767B\u8BB0\uFF0C\u5E76\u5220\u9664\u5B83\u4FDD\u5B58\u5728\u672C\u673A\u7684\u8BA4\u8BC1\u4FE1\u606F\u3002\u64CD\u4F5C\u8BB0\u5F55\u4ECD\u4F1A\u4FDD\u7559\u3002|Remove this registration and its local credentials. Audit records are retained.
\u8FD4\u56DE\u6280\u80FD\u4E2D\u5FC3|Back to capabilities
\u524D\u5F80\u6280\u80FD\u4E2D\u5FC3|Go to capabilities
\u5728\u8FD9\u91CC\u63A5\u5165 Skill\u3001MCP \u548C\u672C\u5730\u5DE5\u5177\uFF1B\u5BFC\u5165\u540E\u7684\u51C6\u5907\u4E0E\u68C0\u67E5\u7ED3\u679C\u4F1A\u663E\u793A\u5728\u4E0B\u65B9\u3002|Import Skills, MCP services and local tools. Preparation and validation results appear below.
\u5148\u9009\u62E9\u80FD\u529B\u7C7B\u578B\uFF0C\u9875\u9762\u53EA\u663E\u793A\u8BE5\u7C7B\u578B\u9700\u8981\u7684\u914D\u7F6E\u3002|Choose a capability type to view its configuration fields.
\u5E38\u89C1\u914D\u7F6E\u90FD\u53EF\u4EE5\u76F4\u63A5\u4F7F\u7528|Common configurations are supported
\u9700\u8981\u4FEE\u6539\u914D\u7F6E\uFF1F\u91CD\u65B0\u5BFC\u5165\u540C\u540D\u80FD\u529B\u4F1A\u521B\u5EFA\u65B0 revision|To update a capability, import the same name to create a new revision.
\u6DFB\u52A0 MCP \u670D\u52A1|Add MCP service
\u8FDE\u63A5 MCP|Connect MCP
\u8FDE\u63A5 MCP \u670D\u52A1|Connect MCP service
\u63A5\u5165\u5916\u90E8\u670D\u52A1\u4E0E\u5DE5\u5177|Connect services and tools
MCP \u670D\u52A1|MCP services
MCP \u914D\u7F6E JSON|MCP configuration JSON
\u53EF\u4EE5\u76F4\u63A5\u7C98\u8D34\u73B0\u6709\u914D\u7F6E\u3002Authorization\u3001Token \u7B49\u654F\u611F\u503C\u4F1A\u5728\u767B\u8BB0\u524D\u4ECE\u914D\u7F6E\u4E2D\u5265\u79BB\uFF0C\u5355\u72EC\u4FDD\u5B58\u4E14\u4E0D\u518D\u56DE\u663E\u3002|Paste an existing configuration. Sensitive values such as Authorization and tokens are extracted before registration, stored separately and never displayed again.
\u652F\u6301\u4E00\u6B21\u6DFB\u52A0\u591A\u4E2A\u670D\u52A1|Add multiple services at once
Token \u4F1A\u5355\u72EC\u4FDD\u5B58\u5230\u672C\u673A\u5BC6\u94A5\u4FDD\u9669\u7BB1\uFF0C\u4E0D\u8FDB\u5165 MCP \u914D\u7F6E\u6216\u5217\u8868 API|Tokens are stored in the local secret store, separately from MCP configuration and list responses.
\u7C98\u8D34 JSON|Paste JSON
\u8F7D\u5165 JSON \u6587\u4EF6|Load JSON file
\u683C\u5F0F\u5316|Format
\u586B\u5165\u793A\u4F8B|Insert example
\u4ECE\u526A\u8D34\u677F\u7C98\u8D34|Paste from clipboard
\u7B49\u5F85\u7C98\u8D34\u914D\u7F6E|Waiting for configuration
\u7C98\u8D34\u540E\u4F1A\u7ACB\u5373\u68C0\u67E5 JSON \u683C\u5F0F\u548C\u670D\u52A1\u6570\u91CF|Pasted JSON is checked for syntax and service count.
\u4E2D\u53EF\u5305\u542B\u591A\u4E2A\u670D\u52A1|may contain multiple services
\u670D\u52A1\u540D\u79F0|Service name
\u586B\u5199\u670D\u52A1\u5730\u5740|Enter service URL
MCP \u670D\u52A1\u5730\u5740|MCP service URL
\u586B\u5199\u6B63\u5728\u8FD0\u884C\u7684 MCP \u5730\u5740|Enter a running MCP service URL
\u6DFB\u52A0 HTTP MCP|Add HTTP MCP
\u9002\u5408\u5DF2\u7ECF\u63D0\u4F9B Streamable HTTP \u5730\u5740\u7684\u670D\u52A1\u3002|For services with an existing Streamable HTTP endpoint.
\u8FD9\u4E2A MCP \u9700\u8981\u8BBF\u95EE\u7F51\u7EDC|This MCP requires network access
\u6DFB\u52A0\u672C\u5730 MCP|Add local MCP
\u9009\u62E9\u672C\u5730 MCP \u9879\u76EE|Select local MCP project
\u5DF2\u6709\u914D\u7F6E\u53EF\u4EE5\u76F4\u63A5\u7C98\u8D34\uFF1B\u4E5F\u53EF\u4EE5\u624B\u52A8\u586B\u5199\u670D\u52A1\u5730\u5740\uFF0C\u6216\u9009\u62E9\u672C\u5730 MCP \u9879\u76EE\u3002|Paste a configuration, enter a service URL, or select a local MCP project.
\u4E09\u79CD\u65B9\u5F0F\u4EFB\u9009\u4E00\u79CD\u3002ExoHarness \u4F1A\u81EA\u52A8\u8BC6\u522B\u5E38\u89C1\u7684 Python\u3001Node.js \u6216 Shell \u542F\u52A8\u65B9\u5F0F\u3002|Choose one input method. ExoHarness detects common Python, Node.js and Shell entrypoints.
\u7B49\u5F85\u9009\u62E9\u542F\u52A8\u6587\u4EF6\u3001ZIP \u6216\u76EE\u5F55|Select an entrypoint, ZIP or directory
\u9009\u62E9\u542F\u52A8\u6587\u4EF6|Select entrypoint
\u9009\u62E9\u9879\u76EE ZIP|Select project ZIP
\u9009\u62E9\u9879\u76EE\u6587\u4EF6\u5939|Select project folder
\u672A\u9009\u62E9 ZIP|No ZIP selected
\u672A\u9009\u62E9\u6587\u4EF6\u5939|No folder selected
\u672A\u9009\u62E9\u76EE\u5F55|No directory selected
\u5165\u53E3\u6A21\u5757\u6216\u6587\u4EF6|Entrypoint module or file
\u89E3\u91CA\u5668\u6216\u7A0B\u5E8F|Interpreter or program
\u5DE5\u5177\u540D\u79F0\u524D\u7F00|Tool name prefix
\u5141\u8BB8\u5168\u90E8\u5DF2\u542F\u7528|Allow all enabled
\u81EA\u52A8\u51C6\u5907\u5E76\u542F\u7528|Prepare and enable automatically
\u4EC5\u6388\u6743|Authorized only
\u51C6\u5907\u8FC7\u7A0B\u4E0D\u4F1A\u6267\u884C\u751F\u6210\u3001\u53D1\u9001\u6D88\u606F\u7B49\u4E1A\u52A1\u64CD\u4F5C|Preparation does not run generation, messaging or other business operations.
\u81EA\u52A8\u51C6\u5907\u5E76\u542F\u7528\uFF1A\u5141\u8BB8\u6807\u51C6\u8FDE\u63A5\u548C\u542F\u52A8\u6743\u9650\uFF0C\u5B89\u88C5\u58F0\u660E\u7684\u53D7\u652F\u6301\u4F9D\u8D56\u5E76\u68C0\u67E5\uFF1B\u4E0D\u8986\u76D6\u5DF2\u62D2\u7EDD\u6743\u9650\uFF0C\u4E0D\u6267\u884C Skill \u5B89\u88C5\u811A\u672C\u3002\u53D6\u6D88\u52FE\u9009\u5219\u4EC5\u767B\u8BB0\u3002|Allow standard connection and startup permissions, prepare supported declared dependencies, and validate readiness. Denied permissions remain unchanged. Skill installation scripts are not executed. Clear this option to register only.
\u6DFB\u52A0 Skill|Add Skill
\u52A0\u5165\u5DE5\u4F5C\u65B9\u6CD5\u4E0E\u914D\u5957\u8D44\u6599|Add instructions and supporting resources
\u6DFB\u52A0\u4E00\u5957\u5DE5\u4F5C\u8BF4\u660E\u548C\u914D\u5957\u8D44\u6599\uFF0CAgent \u4F1A\u5728\u5408\u9002\u7684\u4EFB\u52A1\u4E2D\u6309\u9700\u4F7F\u7528\u3002|Add task instructions and resources for the Agent to load when needed.
\u4E09\u79CD\u65B9\u5F0F\u4EFB\u9009\u4E00\u79CD\u3002\u5355\u4E2A .md \u6587\u4EF6\u4F1A\u81EA\u52A8\u4F5C\u4E3A Skill \u5165\u53E3\uFF1B\u5305\u542B\u53C2\u8003\u8D44\u6599\u6216\u6A21\u677F\u65F6\uFF0C\u76EE\u5F55\u6216 ZIP \u7684\u6839\u76EE\u5F55\u9700\u6709 SKILL.md\u3002|Choose one input method. A single .md file becomes the Skill entrypoint. Folders and ZIP packages with resources must contain SKILL.md at the root.
\u7B49\u5F85\u9009\u62E9 Skill \u6587\u4EF6|Select a Skill file
\u9009\u62E9\u5355\u4E2A Skill Markdown|Select Skill Markdown file
\u9009\u62E9 Skill ZIP|Select Skill ZIP
\u9009\u62E9 Skill \u6587\u4EF6\u5939|Select Skill folder
\u5C1A\u672A\u9009\u62E9\u6587\u4EF6|No file selected
\u672A\u9009\u62E9\u6587\u4EF6|No file selected
\u68C0\u67E5\u5E76\u6DFB\u52A0 Skill|Validate and add Skill
\u6DFB\u52A0\u672C\u5730\u5DE5\u5177|Add local tool
\u63A5\u5165\u811A\u672C\u6216\u4EE3\u7801\u5305|Import script or code package
\u628A\u4E00\u4E2A\u811A\u672C\u6216\u4EE3\u7801\u5305\u6DFB\u52A0\u4E3A Agent \u53EF\u8C03\u7528\u7684\u5DE5\u5177\u3002\u6DFB\u52A0\u5B8C\u6210\u540E\u9700\u8981\u5148\u9A8C\u8BC1\u518D\u542F\u7528\u3002|Register a script or code package as an Agent tool. Validate it before enabling.
\u4E09\u79CD\u65B9\u5F0F\u4EFB\u9009\u4E00\u79CD\u3002\u652F\u6301 Python\u3001Node.js \u548C Shell\uFF1B\u6DFB\u52A0\u8FC7\u7A0B\u4E2D\u4E0D\u4F1A\u8FD0\u884C\u4EE3\u7801\u3002|Choose one input method. Python, Node.js and Shell are supported. Importing does not execute code.
\u53EF\u4EE5\u9009\u62E9\u542F\u52A8\u6587\u4EF6\uFF0C\u4E5F\u53EF\u4EE5\u6DFB\u52A0\u5B8C\u6574\u76EE\u5F55\u6216 ZIP \u5305\u3002|Select an entrypoint, a complete folder or a ZIP package.
\u9009\u62E9\u5DE5\u5177\u811A\u672C|Select tool script
\u9009\u62E9\u4EE3\u7801\u5305 ZIP|Select code ZIP
\u9009\u62E9\u4EE3\u7801\u5305\u6587\u4EF6\u5939|Select code folder
\u4EE3\u7801\u5305\u8FD0\u884C\u65B9\u5F0F|Code runtime
\u5DE5\u5177\u540D\u79F0|Tool name
\u5DE5\u5177\u7EC4\u540D\u79F0|Tool group name
\u5DE5\u5177\u7EC4\u8BF4\u660E|Tool group description
\u8FD9\u4E2A\u670D\u52A1\u80FD\u505A\u4EC0\u4E48\uFF1F|What does this service do?
\u60F3\u8BA9\u5B83\u505A\u4EC0\u4E48\uFF1F|What should it do?
\u8F93\u5165\u53C2\u6570\u683C\u5F0F|Input parameters
\u4F7F\u7528 JSON Schema \u63CF\u8FF0 Agent \u8C03\u7528\u5DE5\u5177\u65F6\u53EF\u4EE5\u586B\u5199\u7684\u53C2\u6570|Describe accepted tool arguments with JSON Schema.
\u9700\u8981\u7684\u6743\u9650\uFF08\u53EF\u9009\uFF09|Required permissions (optional)
\u591A\u4E2A\u6743\u9650\u4F7F\u7528\u9017\u53F7\u5206\u9694|Separate permissions with commas
\u9A8C\u8BC1\u5E76\u6CE8\u518C|Validate and register
\u5BFC\u5165\u5B8C\u6210|Import complete
\u786E\u8BA4\u540E\u4F1A\u521B\u5EFA\u65B0\u7684\u4E0D\u53EF\u53D8\u7248\u672C\uFF1B\u5F53\u524D\u542F\u7528\u72B6\u6001\u548C\u5DF2\u6709\u6743\u9650\u51B3\u5B9A\u4F1A\u4FDD\u7559\u3002|Confirmation creates a new immutable revision while retaining activation status and permission decisions.
\u6309\u80FD\u529B\u7C7B\u578B\u914D\u7F6E\u6A21\u578B\u3002\u542F\u7528\u540E\uFF0CLLM \u8D1F\u8D23\u5BF9\u8BDD\u63A8\u7406\uFF0CTTS \u7B49\u751F\u6210\u6A21\u578B\u53EF\u7531\u7528\u6237\u7AEF\u6309\u9700\u8C03\u7528\u3002|Configure models by capability type. Enabled LLMs handle conversations; TTS and other models are available for appropriate tasks.
\u6A21\u578B\u5217\u8868|Model list
\u6A21\u578B\u7C7B\u578B|Model type
\u6A21\u578B\u540D\u79F0|Model name
\u6A21\u578B\u4FE1\u606F|Model details
\u6A21\u578B\u8BBE\u7F6E|Model settings
\u6A21\u578B|Model
LLM \xB7 \u5927\u8BED\u8A00\u6A21\u578B|LLM \xB7 Language model
TTS \xB7 \u6587\u5B57\u8F6C\u8BED\u97F3|TTS \xB7 Text to speech
ASR \xB7 \u8BED\u97F3\u8F6C\u6587\u5B57|ASR \xB7 Speech to text
\u6DFB\u52A0\u6A21\u578B\u914D\u7F6E|Add model configuration
API \u534F\u8BAE|API protocol
API \u5730\u5740|API endpoint
\u63A5\u53E3\u8FDE\u63A5|Connection
\u63A5\u53E3\u914D\u7F6E\u540D\u79F0|Connection name
\u914D\u7F6E\u540D\u79F0|Configuration name
\u914D\u7F6E\u540D\u79F0\u7528\u4E8E\u533A\u5206\u4FDD\u5B58\u7684\u914D\u7F6E\uFF1B\u6A21\u578B\u540D\u79F0\u4F1A\u76F4\u63A5\u53D1\u9001\u7ED9 API \u670D\u52A1\u3002|The configuration name identifies the saved entry. The model name is sent to the API.
\u586B\u5199\u6A21\u578B\u670D\u52A1\u63D0\u4F9B\u7684\u6A21\u578B\u540D\u79F0|Enter the model name provided by the service
OpenAI \u517C\u5BB9\u63A5\u53E3|OpenAI-compatible API
Anthropic \u63A5\u53E3|Anthropic API
\u4FDD\u6301\u73B0\u6709\u534F\u8BAE|Keep current protocol
\u8FDE\u63A5 LLM \u670D\u52A1\u6240\u9700\u7684\u4FE1\u606F\u3002|Connection details for the LLM service.
\u4E0D\u540C TTS \u63A5\u53E3\u4F7F\u7528\u4E0D\u540C\u540D\u79F0\uFF0C\u4FDD\u5B58\u65B0\u63A5\u53E3\u4E0D\u4F1A\u8986\u76D6\u5DF2\u914D\u7F6E\u7684\u670D\u52A1\u548C\u5BC6\u94A5\u3002|Use a separate name for each TTS connection. Saving a new connection preserves existing services and credentials.
\u7528\u4E8E\u533A\u5206\u591A\u5957\u914D\u7F6E\uFF0C\u652F\u6301\u4E2D\u6587|Identifies configurations; Chinese names are supported
\u7528\u4E8E\u6B63\u5F0F\u7528\u6237\u7AEF\u7684\u5BF9\u8BDD\u4E0E\u4EFB\u52A1|For user conversations and tasks
\u542F\u7528\u8FD9\u4E2A LLM\uFF1B\u4FDD\u5B58\u540E\u81EA\u52A8\u505C\u7528\u5176\u4ED6 LLM|Enable this LLM; saving disables other LLMs
\u5DF2\u63D0\u4F9B\u5E38\u7528\u9ED8\u8BA4\u503C\uFF0C\u70B9\u51FB\u5C55\u5F00\u540E\u53EF\u4EE5\u4FEE\u6539\u3002|Common defaults are provided. Expand to edit them.
LLM \u9AD8\u7EA7\u914D\u7F6E|Advanced LLM settings
\u601D\u8003\u6A21\u5F0F|Reasoning mode
\u5173\u95ED\u601D\u8003|Disable reasoning
\u9ED8\u8BA4\u5173\u95ED|Disabled by default
\u6700\u5927\u8F93\u51FA Token|Maximum output tokens
\u8F93\u51FA\u957F\u5EA6\u63A7\u5236|Output length
\u4E0D\u53D1\u9001\u8F93\u51FA\u4E0A\u9650\uFF08\u7531\u6A21\u578B\u670D\u52A1\u51B3\u5B9A\uFF09|Omit output limit (provider default)
\u4E0D\u53D1\u9001\u4E0A\u9650\u65F6\uFF0C\u6A21\u578B\u670D\u52A1\u81EA\u8EAB\u7684\u8F93\u51FA\u9650\u5236\u4ECD\u7136\u6709\u6548\u3002\u4EC5\u9002\u7528\u4E8E\u652F\u6301\u7701\u7565\u8BE5\u53C2\u6570\u7684\u517C\u5BB9\u63A5\u53E3\u3002|The provider output limit still applies. Available only for APIs that support omitting this parameter.
\u4E0A\u4E0B\u6587\u7A97\u53E3|Context window
\u9ED8\u8BA4 0.7\uFF1BAnthropic \u6700\u5927\u4E3A 1|Default 0.7; maximum 1 for Anthropic
\u9ED8\u8BA4 0.9\uFF0C\u8303\u56F4 0.01\u20131|Default 0.9; range 0.01\u20131
\u9ED8\u8BA4 40\uFF1BOpenAI \u5B98\u65B9\u63A5\u53E3\u4E0D\u53D1\u9001\u6B64\u53C2\u6570|Default 40; omitted for the official OpenAI API
\u706B\u5C71\u5F15\u64CE TTS \xB7 SSE v3|Volcengine TTS \xB7 SSE v3
\u963F\u91CC\u4E91\u5343\u95EE TTS \xB7 \u975E\u5B9E\u65F6 HTTP|Alibaba Qwen TTS \xB7 HTTP
\u963F\u91CC\u4E91 Qwen-Audio TTS \xB7 \u5B9E\u65F6 WebSocket|Alibaba Qwen-Audio TTS \xB7 WebSocket
\u97F3\u8272|Voice
\u670D\u52A1\u5546\u63D0\u4F9B\u7684\u97F3\u8272\u6807\u8BC6|Provider voice identifier
ExoHarness \u4E2D\u7A33\u5B9A\u7684\u58F0\u97F3\u89D2\u8272\u6807\u8BC6|Stable voice profile identifier in ExoHarness
\u8BED\u8A00|Language
\u5BF9\u5E94\u8BF7\u6C42\u4E2D\u7684 language_type\uFF1B\u5355\u4E00\u8BED\u79CD\u5EFA\u8BAE\u660E\u786E\u9009\u62E9|Maps to language_type. Select a language for single-language content.
Auto \xB7 \u81EA\u52A8\u8BC6\u522B|Auto \xB7 Detect language
Chinese \xB7 \u4E2D\u6587|Chinese
English \xB7 \u82F1\u6587|English
Japanese \xB7 \u65E5\u6587|Japanese
Korean \xB7 \u97E9\u6587|Korean
French \xB7 \u6CD5\u6587|French
German \xB7 \u5FB7\u6587|German
Russian \xB7 \u4FC4\u6587|Russian
Italian \xB7 \u610F\u5927\u5229\u6587|Italian
Spanish \xB7 \u897F\u73ED\u7259\u6587|Spanish
Portuguese \xB7 \u8461\u8404\u7259\u6587|Portuguese
\u97F3\u9891\u683C\u5F0F|Audio format
PCM\uFF08\u4FDD\u5B58\u65F6\u5C01\u88C5\u4E3A WAV\uFF09|PCM (saved as WAV)
\u91C7\u6837\u7387|Sample rate
\u5B9E\u65F6 WebSocket \u9ED8\u8BA4 22050 Hz|Default 22050 Hz for real-time WebSocket
OpenAI \u517C\u5BB9 \xB7 \u97F3\u9891\u8F6C\u5F55|OpenAI-compatible \xB7 Audio transcription
Bearer Token\uFF08\u53EF\u9009\uFF09|Bearer token (optional)
\u8BF7\u8F93\u5165 API Key|Enter API key
\u4FDD\u5B58\u914D\u7F6E|Save configuration
\u5F00\u59CB\u6D4B\u8BD5|Test connection
\u5F53\u524D\u542F\u7528\u914D\u7F6E|Active configuration
\u914D\u7F6E ID|Configuration ID
LLM\u3001TTS \u4E0E ASR \u5206\u7EC4\u663E\u793A\uFF1B\u5DF2\u542F\u7528\u6A21\u578B\u53EF\u5728\u7528\u6237\u7AEF\u4F7F\u7528\uFF0C\u70B9\u51FB\u914D\u7F6E\u8F7D\u5165\u5DE6\u4FA7\u4FEE\u6539\u3002|Models are grouped by LLM, TTS and ASR. Enabled models are available in the portal. Select an entry to edit it.
\u6BCF\u79CD\u7C7B\u578B\u53EF\u4EE5\u4FDD\u5B58\u591A\u4E2A\u914D\u7F6E\uFF0C\u5E76\u72EC\u7ACB\u542F\u7528\u4E00\u4E2A\uFF1B\u4E0D\u540C\u7C7B\u578B\u4E92\u4E0D\u5F71\u54CD\u3002|Save multiple configurations for each type and enable one independently.
\u4E3A\u4E0D\u540C\u4EFB\u52A1\u914D\u7F6E\u52A9\u7406|Configure assistants for different tasks
\u72EC\u7ACB\u8BBE\u7F6E\u8EAB\u4EFD\u3001\u53EF\u7528\u80FD\u529B\u548C\u53D1\u5E03\u72B6\u6001\u3002\u80FD\u529B\u9009\u62E9\u7531\u540E\u7AEF\u6267\u884C\u68C0\u67E5\uFF0C\u4E0D\u53EA\u662F\u63D0\u793A\u8BCD\u3002|Configure identity, permissions and publication separately. Capability access is enforced by the backend.
\u521B\u5EFA\u667A\u80FD\u52A9\u7406|Create assistant
\u52A9\u7406\u5217\u8868|Assistant list
\u52A9\u7406|Assistant
\u9009\u62E9\u540E\u7F16\u8F91\u5F53\u524D\u52A9\u7406\uFF1B\u53EA\u6709\u70B9\u51FB\u201C\u521B\u5EFA\u667A\u80FD\u52A9\u7406\u201D\u624D\u4F1A\u65B0\u589E\u3002|Select an assistant to edit it. Create assistant adds a new one.
\u5F62\u8C61\u4E0E\u540D\u79F0|Appearance and name
\u57FA\u672C\u4FE1\u606F|Basic information
\u914D\u7F6E\u7248\u672C|Configuration revision
\u663E\u793A\u540D\u79F0|Display name
\u521B\u5EFA\u540E\u4E0D\u53EF\u4FEE\u6539|Immutable after creation
\u7528\u4E8E\u6B63\u5F0F\u7528\u6237\u7AEF\u7684\u65B0\u5BF9\u8BDD|For new portal conversations
\u8BBE\u4E3A\u65B0\u5BF9\u8BDD\u9ED8\u8BA4\u52A9\u7406|Default assistant for new conversations
\u5934\u50CF|Avatar
\u5934\u50CF\u9884\u89C8|Avatar preview
\u9009\u62E9\u5934\u50CF|Choose avatar
\u79FB\u9664\u56FE\u7247|Remove image
\u6587\u5B57\u5934\u50CF|Text avatar
PNG\u3001JPEG \u6216 WebP\uFF0C\u6700\u5927 2 MiB|PNG, JPEG or WebP, up to 2 MiB
\u672A\u9009\u62E9\u56FE\u7247\u6216\u56FE\u7247\u65E0\u6CD5\u52A0\u8F7D\u65F6\u663E\u793A|Displayed when no image is selected or loading fails
\u5934\u50CF\u4E0A\u4F20\u540E\u6309\u5185\u5BB9\u4FDD\u5B58\uFF1B\u65E7\u4F1A\u8BDD\u4ECD\u4FDD\u7559\u521B\u5EFA\u65F6\u7684\u52A9\u7406\u5FEB\u7167\u3002|Avatars are saved by content. Existing conversations retain their original assistant snapshot.
\u7B80\u77ED\u8BF4\u660E|Short description
\u7528\u6237\u5728\u5BF9\u8BDD\u754C\u9762\u770B\u5230\u7684\u52A9\u7406\u8BF4\u660E|Assistant description shown in chat
\u8EAB\u4EFD\u63D0\u793A\u8BCD|Identity prompt
\u8F93\u5165\u667A\u80FD\u52A9\u7406\u7684\u8EAB\u4EFD\u3001\u804C\u8D23\u3001\u8FB9\u754C\u4E0E\u4EA4\u4E92\u98CE\u683C|Enter the assistant identity, responsibilities, limits and communication style
\u5B9A\u4E49\u52A9\u7406\u662F\u8C01\u3001\u670D\u52A1\u5BF9\u8C61\u3001\u80FD\u529B\u8FB9\u754C\u548C\u8868\u8FBE\u65B9\u5F0F\uFF1B\u5177\u4F53\u80FD\u529B\u77E5\u8BC6\u5E94\u653E\u5165 Skill\u3002|Define identity, audience, scope and communication style. Put task-specific instructions in Skills.
\u8EAB\u4EFD\u63D0\u793A\u8BCD\u5C5E\u4E8E\u90E8\u7F72\u914D\u7F6E\uFF0C\u4E0D\u4F1A\u5199\u5165 Harness \u4EE3\u7801\u3002|The identity prompt is deployment configuration, separate from Harness code.
\u6309\u7C7B\u522B\u9009\u62E9\u52A9\u7406\u53EF\u4EE5\u4F7F\u7528\u7684\u80FD\u529B\uFF0C\u4FEE\u6539\u540E\u4FDD\u5B58\u751F\u6548\u3002|Select authorized capabilities by category and save changes.
Skill \u8303\u56F4|Skill scope
MCP \u8303\u56F4|MCP scope
Tool \u8303\u56F4|Tool scope
MCP \u9009\u62E9|MCP selection
Tool \u9009\u62E9|Tool selection
\u6A21\u578B\u80FD\u529B|Model capabilities
\u6A21\u578B\u80FD\u529B\u8303\u56F4|Model capability scope
\u6A21\u578B\u80FD\u529B\u9009\u62E9|Model capability selection
\u5168\u90E8\u5DF2\u542F\u7528|All enabled
\u6307\u5B9A\u80FD\u529B|Selected capabilities
\u4E0D\u4F7F\u7528|None
\u53EA\u770B\u5DF2\u9009|Selected only
\u641C\u7D22\u80FD\u529B\u540D\u79F0|Search capability names
\u641C\u7D22\u540D\u79F0\u3001\u63CF\u8FF0\u6216 ID|Search name, description or ID
\u6309\u9700\u8BFB\u53D6\u5DF2\u6388\u6743\u7684\u6280\u80FD\uFF1B\u9009\u62E9\u201C\u6307\u5B9A\u80FD\u529B\u201D\u540E\uFF0C\u53EA\u6709\u52FE\u9009\u7684\u6280\u80FD\u53EF\u4F9B\u6B64\u52A9\u7406\u4F7F\u7528\u3002|Authorized Skills are loaded on demand. Selected capabilities limits access to checked Skills.
MCP \u9ED8\u8BA4\u4E0D\u6388\u6743\u3002\u52FE\u9009\u5E76\u4FDD\u5B58\u540E\u624D\u5141\u8BB8\u8C03\u7528\uFF1B\u5168\u5C40\u542F\u7528\u4E0D\u4EE3\u8868\u5DF2\u5206\u914D\u7ED9\u52A9\u7406\u3002|MCP access requires explicit selection and saving. Globally enabled services are not automatically assigned to an assistant.
\u9009\u62E9\u6B64\u52A9\u7406\u53EF\u8C03\u7528\u7684\u5DF2\u767B\u8BB0\u5DE5\u5177\u3002\u6267\u884C\u4ECD\u9075\u5B88\u7CFB\u7EDF\u6743\u9650\u4E0E\u6C99\u76D2\u9650\u5236\u3002|Choose registered tools for this assistant. System permissions and sandbox limits still apply.
\u5355\u72EC\u6388\u6743 TTS\u3001ASR\u3001\u56FE\u7247\u751F\u6210\u7B49\u6267\u884C\u80FD\u529B\uFF1B\u4E0D\u51B3\u5B9A\u5BF9\u8BDD LLM\u3002\u5168\u90E8\u5DF2\u542F\u7528\u4F1A\u81EA\u52A8\u5305\u542B\u540E\u7EED\u65B0\u589E\u4E14\u5DF2\u63A5\u901A\u7684\u6A21\u578B\uFF1B\u6307\u5B9A\u80FD\u529B\u53EA\u5141\u8BB8\u52FE\u9009\u9879\u3002|Authorize TTS, ASR, image generation and other model capabilities separately from the chat LLM. All enabled includes future connected models; Selected capabilities includes checked entries only.
\u53EF\u5173\u8054\u6280\u80FD\u5217\u8868|Related Skills
\u91CD\u65B0\u8BFB\u53D6\u5DE5\u5177\u5217\u8868|Reload tools
\u9ED8\u8BA4\u5BF9\u8BDD\u6A21\u578B|Default chat model
\u4F7F\u7528\u5F53\u524D\u6A21\u578B|Use current model
\u8DDF\u968F\u5168\u5C40\u9ED8\u8BA4|Use global default
\u7528\u4E8E\u7406\u89E3\u4E0E\u56DE\u7B54\u7684 LLM\u3002TTS\u3001ASR\u3001\u56FE\u7247\u751F\u6210\u7B49\u6267\u884C\u6743\u9650\u8BF7\u5728\u201C\u80FD\u529B\u8303\u56F4 \u2192 \u6A21\u578B\u80FD\u529B\u201D\u4E2D\u5355\u72EC\u914D\u7F6E\u3002|Select the LLM for conversations. Configure TTS, ASR and image permissions under Capability scope \u2192 Model capabilities.
\u8F6F\u4EF6\u5DE5\u7A0B \xB7 \u79C1\u6709\u9879\u76EE\u6C99\u76D2|Coding \xB7 Private project sandbox
\u542F\u7528\u8F6F\u4EF6\u5DE5\u7A0B|Enable coding
\u5141\u8BB8\u6B64\u52A9\u7406\u5728\u7528\u6237\u79C1\u6709\u9879\u76EE\u4E2D\u8BFB\u5199\u4EE3\u7801\u3001\u8FD0\u884C\u6784\u5EFA\u548C\u6D4B\u8BD5\u3001\u67E5\u770B\u5DEE\u5F02\u5E76\u6062\u590D\u68C0\u67E5\u70B9\u3002\u4E0D\u4F9D\u8D56\u7279\u5B9A Skill\uFF1B\u4EC5\u5728\u4EFB\u52A1\u9700\u8981\u65F6\u52A0\u8F7D\u5DE5\u5177\u3002|Allow code editing, builds, tests, diffs and checkpoint recovery in private projects. Tools are loaded as needed, independently of any Skill.
\u9694\u79BB\u8FD0\u884C\u73AF\u5883|Isolated runtime
\u8BF7\u9009\u62E9\u5DF2\u542F\u7528\u7684\u6C99\u76D2\u670D\u52A1|Select an enabled sandbox service
\u4EC5\u6388\u6743\u8BE5\u670D\u52A1\u58F0\u660E\u7684\u8F6F\u4EF6\u5DE5\u7A0B\u5DE5\u5177\uFF0C\u4E0D\u4F1A\u81EA\u52A8\u6388\u4E88\u5176\u4ED6 MCP \u5DE5\u5177\u3002\u6267\u884C\u59CB\u7EC8\u5728\u9694\u79BB\u5BB9\u5668\u4E2D\uFF0C\u4E0D\u80FD\u8BBF\u95EE\u5BBF\u4E3B\u673A\u3001\u51ED\u636E\u6216\u7F51\u7EDC\uFF1B\u989D\u5916\u4F9D\u8D56\u7531\u7BA1\u7406\u5458\u51C6\u5907\u3002|Only the service's declared coding tools are authorized. Execution is isolated from the host, credentials and network. Administrators prepare additional dependencies.
\u8FD0\u884C\u4E0E\u7528\u6237\u7AEF\u53D1\u5E03|Activation and publication
\u8FD0\u884C\u4E0E\u53D1\u5E03|Activation and publication
\u542F\u7528\u8FD9\u4E2A\u667A\u80FD\u52A9\u7406|Enable this assistant
\u53D1\u5E03\u5230\u7528\u6237\u7AEF\uFF0C\u5141\u8BB8\u6CE8\u518C\u7528\u6237\u5728\u65B0\u5BF9\u8BDD\u4E2D\u9009\u62E9|Publish for registered users in new conversations
\u8EAB\u4EFD\u4E0E\u4EFB\u52A1\u6307\u5F15\u4FDD\u7559\u5728\u4F1A\u8BDD\u5FEB\u7167\u4E2D\uFF1B\u6536\u7D27\u80FD\u529B\u8303\u56F4\u4F1A\u5BF9\u540E\u7EED\u8C03\u7528\u548C\u6062\u590D\u68C0\u67E5\u751F\u6548\u3002\u505C\u7528\u52A9\u7406\u4F1A\u7981\u6B62\u7EE7\u7EED\u8C03\u7528\u80FD\u529B\uFF0C\u4E0D\u4F1A\u5220\u9664\u5DF2\u6709\u6210\u679C\u3002|Identity and task instructions are saved in the conversation snapshot. Restricted permissions apply to future calls and recovery. Disabling an assistant blocks further calls while preserving existing results.
\u5BF9\u8BDD\u63A8\u7406\u4F7F\u7528\u7CFB\u7EDF\u6A21\u578B\u914D\u7F6E\uFF1B\u5E76\u53D1\u4E0E\u9884\u7B97\u7EE7\u7EED\u670D\u4ECE\u7CFB\u7EDF\u4E0A\u9650\u3002\u9ED8\u8BA4\u52A9\u7406\u59CB\u7EC8\u5728\u7528\u6237\u7AEF\u53EF\u7528\u3002|Conversations use system model configuration and concurrency and budget limits. The default assistant is available in the portal.
\u8D44\u6599\u5E93|Knowledge library
\u8D44\u6599\u5206\u7C7B|Categories
\u5206\u7C7B|Category
\u8D44\u6599\u540D\u79F0|Document name
\u8D44\u6599\u5185\u5BB9|Document content
\u8D44\u6599\u5E93\u8BF4\u660E\uFF08\u53EF\u9009\uFF09|Library description (optional)
\u7B2C\u4E00\u6B65 \xB7 \u65B0\u5EFA\u8D44\u6599\u5E93|Step 1 \xB7 Create library
\u7B2C\u4E8C\u6B65 \xB7 \u5206\u7C7B\u4E0E\u5BFC\u5165|Step 2 \xB7 Categorize and import
\u7B2C\u4E09\u6B65 \xB7 \u6388\u6743\u52A9\u7406|Step 3 \xB7 Authorize assistants
\u5148\u5EFA\u7ACB\u8D44\u6599\u5E93\uFF0C\u518D\u5206\u7C7B\u5E76\u5BFC\u5165\u6587\u4EF6\uFF0C\u6700\u540E\u6388\u6743\u52A9\u7406\u5728\u76F8\u5173\u5BF9\u8BDD\u4E2D\u68C0\u7D22\u4F7F\u7528\u3002\u7528\u6237\u79C1\u4EBA\u8D44\u6599\u72EC\u7ACB\u4FDD\u5B58\u3002|Create a library, organize and import documents, then authorize assistants to retrieve them. Personal documents are stored separately.
\u5148\u9009\u62E9\u8D44\u6599\u5E93|Select a library first
\u4FDD\u5B58\u8D44\u6599\u5E93|Save library
\u5220\u9664\u8D44\u6599\u5E93|Delete library
\u65B0\u5EFA|New
\u6BCF\u4E2A\u8D44\u6599\u5E93\u53EF\u5355\u72EC\u6388\u6743\u52A9\u7406\u3002|Each library has separate assistant permissions.
\u5206\u7C7B\u53EF\u9009\u3002\u5148\u9009\u62E9\u5206\u7C7B\uFF0C\u518D\u5BFC\u5165\u6587\u4EF6\uFF1B\u65B0\u5EFA\u5206\u7C7B\u65F6\u53EF\u9009\u4E0A\u7EA7\uFF0C\u6700\u591A\u4E09\u7EA7\u3002|Categories are optional. Select a category before importing. New categories support up to three levels.
\u8BF7\u5148\u4FDD\u5B58\u4E0A\u65B9\u8D44\u6599\u5E93\uFF1B\u4FDD\u5B58\u540E\u5373\u53EF\u521B\u5EFA\u5206\u7C7B\u5E76\u5BFC\u5165\u6587\u4EF6\u3002|Save the library before adding categories or importing files.
\u9009\u62E9\u6587\u4EF6\u5BFC\u5165|Import files
\u67E5\u770B\u4E0E\u7F16\u8F91\u5206\u7C7B|View and edit categories
\u7BA1\u7406\u5206\u7C7B\uFF08\u53EF\u9009\uFF09|Manage categories (optional)
\u5206\u7C7B\u540D\u79F0|Category name
\u8F93\u5165\u5206\u7C7B\u540D\u79F0|Enter category name
\u65B0\u5206\u7C7B\u540D\u79F0|New category name
\u521B\u5EFA\u5E76\u9009\u4E2D|Create and select
\u4E0A\u7EA7\u5206\u7C7B|Parent category
\u9876\u5C42\u5206\u7C7B|Root category
\u9876\u5C42\u5206\u7C7B\uFF08\u7B2C\u4E00\u7EA7\uFF09|Root category (level 1)
\u4FDD\u5B58\u5206\u7C7B|Save category
\u672A\u5206\u7C7B|Uncategorized
\u624B\u52A8\u6DFB\u52A0\u6587\u5B57\u8D44\u6599|Add text document
\u6807\u9898|Title
\u6B63\u6587|Content
\u6807\u7B7E\u4E0E\u6765\u6E90\uFF08\u53EF\u9009\uFF09|Tags and source (optional)
\u6807\u7B7E\uFF08\u9017\u53F7\u5206\u9694\uFF0C\u6700\u591A 8 \u4E2A\uFF09|Tags (comma-separated, up to 8)
\u6765\u6E90\u94FE\u63A5\uFF08\u53EF\u9009\uFF0C\u4E0D\u81EA\u52A8\u6293\u53D6\uFF09|Source URL (optional, not fetched)
\u6765\u6E90\u94FE\u63A5\uFF08\u53EF\u9009\uFF0C\u4EC5\u8BB0\u5F55\u51FA\u5904\uFF0C\u4E0D\u81EA\u52A8\u6293\u53D6\uFF09|Source URL (optional, recorded only)
\u4FDD\u5B58\u8D44\u6599|Save document
\u53EA\u5141\u8BB8\u5DF2\u542F\u7528\u7684\u52A9\u7406\u4F7F\u7528\u672C\u5E93\uFF1B\u4FEE\u6539\u6388\u6743\u540E\uFF0C\u65B0\u8BF7\u6C42\u7ACB\u5373\u751F\u6548\u3002|Only enabled assistants can use this library. Permission changes apply to new requests immediately.
\u4FDD\u5B58\u6388\u6743|Save permissions
\u652F\u6301 DOCX\u3001\u6587\u5B57\u578B PDF\uFF08\u6700\u5927 4 MB\uFF09\uFF0C\u4EE5\u53CA UTF-8 \u6587\u5B57\u6587\u4EF6\uFF08\u6700\u5927 2 MB\uFF09\u3002DOCX \u8868\u683C\u5B8C\u6574\u4FDD\u7559\uFF0CPDF \u4E0D\u786E\u5B9A\u7684\u8868\u683C\u4FDD\u7559\u7248\u9762\u6587\u5B57\u5E76\u63D0\u793A\uFF1B\u6682\u4E0D\u652F\u6301\u626B\u63CF\u4EF6\u6216\u56FE\u7247 OCR\u3002\u63D0\u53D6\u6587\u5B57\u6700\u591A 1 MB\u3002|Supports DOCX and text PDFs (up to 4 MB), and UTF-8 text files (up to 2 MB). DOCX tables are preserved. Uncertain PDF tables retain layout text with a warning. Scans and OCR are not supported. Extracted text is limited to 1 MB.
\u652F\u6301 DOCX\u3001\u6587\u5B57\u578B PDF\uFF08\u6700\u5927 4 MB\uFF09\uFF0C\u4EE5\u53CA UTF-8 \u6587\u5B57\u6587\u4EF6\uFF08\u6700\u5927 2 MB\uFF09\uFF1B\u6BCF\u6B21\u6700\u591A 10 \u4E2A\u3002DOCX \u8868\u683C\u5B8C\u6574\u4FDD\u7559\uFF0CPDF \u4E0D\u786E\u5B9A\u7684\u8868\u683C\u4EC5\u4FDD\u7559\u7248\u9762\u6587\u5B57\u5E76\u63D0\u793A\u3002\u6682\u4E0D\u652F\u6301\u626B\u63CF\u4EF6\u6216\u56FE\u7247 OCR\u3002\u63D0\u53D6\u6587\u5B57\u6700\u591A 1 MB\u3002|Supports up to 10 DOCX or text PDF files (4 MB each), or UTF-8 text files (2 MB each). DOCX tables are preserved. Uncertain PDF tables retain layout text with a warning. Scans and OCR are not supported. Extracted text is limited to 1 MB.
\u67E5\u770B\u5B8C\u6574\u8C03\u7528\u8DEF\u5F84\u3001\u5404\u9636\u6BB5\u8017\u65F6\u4EE5\u53CA\u8131\u654F\u540E\u7684\u8F93\u5165\u4E0E\u8FD4\u56DE\u6570\u636E\u3002|Inspect the call flow, timing, and redacted inputs and outputs.
\u4E8B\u4EF6\u65F6\u95F4\u7EBF|Event timeline
\u4E8B\u4EF6\u5C55\u793A\u4E0E\u7FFB\u9875|Timeline and pagination
\u6574\u7406\u89C6\u56FE|Grouped view
\u539F\u59CB\u4E8B\u4EF6|Raw events
\u6700\u65B0\u4E8B\u4EF6|Latest events
\u5F53\u524D\u663E\u793A\u90E8\u5206\u4E8B\u4EF6\uFF0C\u53EF\u7FFB\u9875\u6216\u8DF3\u5230\u6700\u65B0\u4E8B\u4EF6\uFF1B\u8FD0\u884C\u72B6\u6001\u4E0E\u8C03\u7528\u7ED3\u679C\u4E0D\u53D7\u5206\u9875\u5F71\u54CD\u3002|A page of events is shown. Browse pages or jump to the latest events. Pagination does not affect execution state or results.
\u8FDE\u7EED\u6587\u5B57\u5408\u5E76\u5C55\u793A\uFF0C\u539F\u59CB\u4E8B\u4EF6\u5B8C\u6574\u4FDD\u7559|Text chunks are grouped; original events are retained
\u6A21\u578B\u8BF7\u6C42\u8BC1\u636E|Model request details
\u9010\u6B21\u8BF7\u6C42\u663E\u793A\u5B9E\u9645 Token\uFF0C\u5E76\u5BF9\u7167\u53D1\u9001\u524D\u7684\u4E0A\u4E0B\u6587\u4F30\u7B97|Compare reported tokens with context estimates for each request
\u9009\u62E9\u8BF7\u6C42\u7EC4\u6210\u90E8\u5206|Select request component
\u8C03\u7528\u8DEF\u5F84|Call flow
\u70B9\u51FB\u80FD\u529B\u6216\u7ED3\u679C\u8282\u70B9\u68C0\u67E5\u6570\u636E|Select a tool or result node to inspect data
\u8C03\u7528\u6570\u636E|Call data
\u8C03\u7528\u8BE6\u60C5|Call details
\u5173\u95ED\u8C03\u7528\u8BE6\u60C5|Close call details
\u8C03\u7528\u53C2\u6570|Arguments
\u8FD4\u56DE\u7ED3\u679C|Result
\u8BB0\u5F55\u7C7B\u578B|Record type
\u641C\u7D22\u80FD\u529B\u6216\u64CD\u4F5C|Search capabilities or operations
\u67E5\u770B\u80FD\u529B\u7684\u6DFB\u52A0\u3001\u6743\u9650\u3001\u9A8C\u8BC1\u3001\u8C03\u7528\u4E0E\u542F\u505C\u8BB0\u5F55\u3002|View registration, permission, validation, call and activation records.
\u7BA1\u7406\u8FD0\u884C\u8C03\u5EA6\u3001\u5171\u4EAB\u8D44\u6E90\u3001\u767B\u5F55\u5BC6\u7801\u4E0E\u6CE8\u518C\u901A\u77E5\u3002|Manage scheduling, shared resources, passwords and registration notifications.
\u8FD0\u884C\u4E0E\u8D44\u6E90\u8C03\u5EA6|Scheduling and resources
\u5168\u5C40 Agent \u5E76\u53D1|Global Agent concurrency
\u5355\u4E2A\u7528\u6237\u6D3B\u8DC3 Run|Active runs per user
\u6240\u6709\u7528\u6237\u4E0E\u7BA1\u7406\u7AEF\u4EFB\u52A1\u5171\u4EAB\u3002\u9ED8\u8BA4 1000\u3002|Shared by all users and administrator tasks. Default: 1000.
\u8DE8\u4E0D\u540C\u4F1A\u8BDD\u5E76\u884C\uFF1B\u540C\u4E00\u4F1A\u8BDD\u4E0D\u5E76\u884C\u3002\u9ED8\u8BA4 2\u3002|Different conversations may run concurrently. Turns in one conversation are sequential. Default: 2.
\u540C\u4E00\u4F1A\u8BDD\u59CB\u7EC8\u6309\u987A\u5E8F\u6267\u884C\uFF1B\u4E0D\u540C\u4F1A\u8BDD\u53EF\u5728\u8D26\u53F7\u4E0E\u5168\u5C40\u4E0A\u9650\u5185\u5E76\u884C\u3002\u5171\u4EAB\u8D44\u6E90\u6C60\u53EF\u8BA9\u591A\u4E2A MCP \u5171\u540C\u7ADE\u4E89\u540C\u4E00\u7EC4\u8BBE\u5907\u3002|Turns run sequentially within a conversation. Other conversations share account and global limits. Multiple MCP services can share a resource pool.
MCP \u8D44\u6E90\u58F0\u660E|MCP resource declarations
\u5171\u4EAB\u8D44\u6E90\u6C60|Shared resource pools
\u6DFB\u52A0\u8D44\u6E90\u6C60|Add resource pool
\u672A\u5206\u914D\u8D44\u6E90\u6C60\u7684 MCP \u7EE7\u7EED\u4F7F\u7528\u81EA\u8EAB Manifest \u4E2D\u7684\u5E76\u53D1\u9650\u5236\u3002|MCP services without a pool use their manifest concurrency limits.
\u4F8B\u5982\u591A\u9879 MCP \u5171\u7528\u540C\u4E00\u5757 GPU \u65F6\uFF0C\u5C06\u5B83\u4EEC\u6307\u5411\u540C\u4E00\u4E2A\u6C60\uFF1B\u6C60\u7684\u5E76\u53D1\u8BBE\u4E3A\u8BBE\u5907\u53EF\u5B89\u5168\u627F\u8F7D\u7684\u4EFB\u52A1\u6570\u3002|Assign MCP services using the same GPU to one pool. Set its concurrency to the device's safe capacity.
\u4FDD\u5B58\u8C03\u5EA6\u914D\u7F6E|Save scheduling
\u751F\u6210\u6587\u4EF6\u4E0E\u4E0B\u8F7D|Generated files and downloads
\u89C6\u9891\u3001\u97F3\u9891\u7B49\u751F\u6210\u6587\u4EF6\u7531 ExoHarness \u4FDD\u5B58\uFF0C\u64AD\u653E\u4E0E\u4E0B\u8F7D\u4F7F\u7528\u540C\u4E00\u6709\u6548\u671F\uFF1B\u65E0\u9700\u4F9D\u8D56\u539F\u670D\u52A1\u7684\u4E34\u65F6\u94FE\u63A5\u3002\u6587\u4EF6\u4ECD\u4EC5\u5411\u6709\u6743\u67E5\u770B\u8BE5\u5BF9\u8BDD\u7684\u7528\u6237\u5F00\u653E\u3002|ExoHarness stores generated files with a shared playback and download lifetime, independently of upstream temporary URLs. Conversation access permissions still apply.
\u8BBF\u95EE\u6709\u6548\u671F|Access lifetime
\u6C38\u4E45\u6709\u6548\uFF08\u9ED8\u8BA4\uFF09|No expiry (default)
\u6307\u5B9A\u5929\u6570|Custom duration
\u6709\u6548\u5929\u6570|Days valid
\u4FDD\u5B58\u6587\u4EF6\u914D\u7F6E|Save file settings
\u8BBE\u7F6E\u5BF9\u5DF2\u4FDD\u5B58\u548C\u65B0\u751F\u6210\u7684\u6587\u4EF6\u7ACB\u5373\u751F\u6548\uFF0C\u4ECE\u6587\u4EF6\u767B\u8BB0\u65F6\u95F4\u8BA1\u7B97\u3002\u5230\u671F\u53EA\u9650\u5236\u8BBF\u95EE\uFF0C\u4E0D\u5220\u9664\u6587\u4EF6\uFF1B\u6539\u56DE\u6C38\u4E45\u6709\u6548\u540E\u53EF\u7EE7\u7EED\u8BBF\u95EE\u3002\u5220\u9664\u5BF9\u8BDD\u65F6\uFF0C\u5176\u751F\u6210\u6587\u4EF6\u4E5F\u4F1A\u5220\u9664\u3002|Applies to saved and new files from their registration time. Expiry restricts access without deleting files. Setting no expiry restores access. Deleting a conversation also deletes its generated files.
\u4FEE\u6539\u767B\u5F55\u5BC6\u7801|Change sign-in password
\u9009\u62E9\u7528\u6237|Select user
\u7BA1\u7406\u5458\u62E5\u6709\u5168\u90E8\u914D\u7F6E\u4E0E\u7BA1\u7406\u6743\u9650\u3002|Administrators have full configuration and management access.
\u65B0\u5BC6\u7801|New password
\u786E\u8BA4\u65B0\u5BC6\u7801|Confirm new password
\u66F4\u65B0\u5BC6\u7801|Update password
\u66F4\u65B0\u7BA1\u7406\u5458\u5BC6\u7801|Update administrator password
\u66F4\u65B0\u540E\uFF0C\u8BE5\u7528\u6237\u7C7B\u578B\u5DF2\u6709\u7684\u767B\u5F55\u4F1A\u8BDD\u4F1A\u7ACB\u5373\u9000\u51FA\u3002|Existing sessions for this user type are signed out immediately.
\u6CE8\u518C\u90AE\u4EF6|Registration email
\u90AE\u4EF6\u6807\u9898|Email subject
\u90AE\u4EF6\u6B63\u6587|Email body
\u4FDD\u5B58\u90AE\u4EF6\u5185\u5BB9|Save email content
\u8BBE\u7F6E\u65B0\u7528\u6237\u6536\u5230\u7684\u9A8C\u8BC1\u7801\u90AE\u4EF6\u6807\u9898\u548C\u6B63\u6587\u3002\u4FDD\u5B58\u540E\uFF0C\u65B0\u53D1\u9001\u7684\u90AE\u4EF6\u4F1A\u7ACB\u5373\u4F7F\u7528\u8FD9\u91CC\u7684\u5185\u5BB9\u3002|Configure the subject and body of verification emails. New emails use the saved content immediately.
\u662F\u5FC5\u586B\u7684\u9A8C\u8BC1\u7801\u4F4D\u7F6E\uFF1B\u90AE\u4EF6\u6309\u7EAF\u6587\u672C\u53D1\u9001\u3002|is the required verification-code placeholder. Emails are sent as plain text.
\u67E5\u770B\u5BF9\u8BDD\u8BB0\u5F55\uFF0C\u8BBE\u7F6E\u9ED8\u8BA4\u52A9\u7406\u3001\u80FD\u529B\u6743\u9650\u548C\u7528\u6237\u5B89\u88C5\u7684\u80FD\u529B\u3002|View conversations and manage assigned assistants, permissions and installed capabilities.
\u6635\u79F0\u3001\u7528\u6237\u540D\u6216\u90AE\u7BB1|Nickname, username or email
\u9009\u62E9\u4E00\u4E2A\u7528\u6237|Select a user
\u7528\u6237\u8BE6\u60C5\u3001\u9ED8\u8BA4\u52A9\u7406\u548C\u6267\u884C\u5BF9\u8BDD\u4F1A\u663E\u793A\u5728\u8FD9\u91CC\u3002|User details, assigned assistant and conversations appear here.
\u9ED8\u8BA4\u52A9\u7406|Default assistant
\u4FEE\u6539\u52A9\u7406\u53EA\u5F71\u54CD\u65B0\u4F1A\u8BDD\uFF1B\u5173\u95ED\u80FD\u529B\u4F7F\u7528\u6743\u9650\u4F1A\u963B\u6B62\u8BE5\u7528\u6237\u65B0\u7684\u5BF9\u5E94\u8C03\u7528\u3002\u7528\u6237\u5B89\u88C5\u7684\u80FD\u529B\u9ED8\u8BA4\u79C1\u6709\uFF0C\u4EC5\u4E0D\u542B\u51ED\u636E\u3001\u4E0D\u53EF\u6267\u884C\u7684\u901A\u7528 Skill \u53EF\u7531\u7BA1\u7406\u5458\u6309\u7528\u6237\u5206\u4EAB\uFF1B\u5206\u4EAB\u4E0D\u8D8A\u8FC7\u52A9\u7406\u6388\u6743\u3002\u5B8C\u6574\u6267\u884C\u8BB0\u5F55\u53EF\u8FDB\u5165 Trace\u3002|Assistant changes apply to new conversations. Revoked capability access blocks new calls. Installed capabilities are private by default; administrators may share non-executable Skills without credentials. Assistant permissions still apply. Execution records are available in Trace.
\u7BA1\u7406\u5F53\u524D\u52A9\u7406|Manage current assistant
\u52A9\u7406\u914D\u7F6E|Assistant configuration
\u9009\u62E9\u7528\u6237\u67E5\u770B\u5BF9\u8BDD\u4E0E\u52A9\u7406\u914D\u7F6E|Select a user to view conversations and assistant configuration
\u4ECA\u5929\u60F3\u4E00\u8D77\u505A\u4EC0\u4E48\uFF1F|What would you like to create?
\u5236\u4F5C\u5BA3\u4F20\u89C6\u9891\u3001\u8BBE\u8BA1\u6D77\u62A5\u3001\u521B\u4F5C\u97F3\u4E50\uFF0C\u6216\u64B0\u5199\u6587\u6863\u4E0E PPT\u2014\u2014\u544A\u8BC9\u6211\u4F60\u7684\u4E3B\u9898\u548C\u8981\u6C42\u3002|Create promotional videos, posters, music, documents or presentations. Tell me your topic and requirements.
\u4ECE\u4E00\u6BB5\u7075\u611F\u51FA\u53D1\uFF0C\u521B\u4F5C\u89C6\u9891\u3001\u97F3\u4E50\u3001\u56FE\u50CF\u4E0E\u6587\u6863\u3002\u4F60\u7684\u4E0B\u4E00\u4EF6\u4F5C\u54C1\uFF0C\u4ECE\u8FD9\u91CC\u5F00\u59CB\u3002|Turn an idea into videos, music, images or documents. Start your next creation here.
\u521B\u4F5C|Create
\u60F3\u8C61|Imagine
\u65B0\u5BF9\u8BDD|New chat
\u65B0\u4F1A\u8BDD \xB7 \u53D1\u9001\u540E\u81EA\u52A8\u4FDD\u5B58|New chat \xB7 Saved after your first message
\u5BF9\u8BDD|Chat
\u5BF9\u8BDD\u5BFC\u822A|Chat navigation
\u5BF9\u8BDD\u8BB0\u5F55|Chat history
\u641C\u7D22\u5386\u53F2\u5BF9\u8BDD|Search chat history
\u5F00\u59CB\u65B0\u5BF9\u8BDD|Start new chat
\u5173\u95ED\u5BF9\u8BDD\u8BB0\u5F55|Close chat history
\u6298\u53E0\u5BF9\u8BDD\u4FA7\u680F|Collapse sidebar
\u5C55\u5F00\u5BF9\u8BDD\u4FA7\u680F|Expand sidebar
\u8D26\u53F7\u83DC\u5355|Account menu
\u6211\u7684\u8D26\u53F7|My account
\u7CFB\u7EDF\u914D\u7F6E|System configuration
\u6A21\u578B\u4E0E\u80FD\u529B\u7531\u7BA1\u7406\u5458\u7EDF\u4E00\u914D\u7F6E|Models and capabilities are managed by the administrator
\u6DFB\u52A0\u53C2\u8003\u6587\u4EF6|Attach reference files
\u6DFB\u52A0\u6587\u4EF6|Attach files
\u53D1\u9001\u6D88\u606F|Send message
\u53D1\u9001\u6D88\u606F\u2026|Send message\u2026
\u505C\u6B62|Stop
\u505C\u6B62\u5F53\u524D\u8FD0\u884C|Stop current run
AI \u751F\u6210\u5185\u5BB9\u53EF\u80FD\u6709\u8BEF\uFF0C\u8BF7\u6838\u5BF9\u91CD\u8981\u4FE1\u606F|AI-generated content may be inaccurate. Verify important information.
Enter \u53D1\u9001 \xB7 Shift + Enter \u6362\u884C|Enter to send \xB7 Shift + Enter for a new line
\u91CA\u653E\u6587\u4EF6\u6DFB\u52A0\u5230\u5BF9\u8BDD|Drop files to attach to chat
\u52A0\u8F7D\u66F4\u65E9\u7684\u6D88\u606F|Load earlier messages
\u6EDA\u52A8\u5230\u6700\u65B0\u6D88\u606F|Scroll to latest message
\u56DE\u5230\u6700\u65B0\u6D88\u606F|Back to latest message
\u4E2A\u4EBA\u8D44\u6599|Profile
\u6211\u7684\u80FD\u529B|My capabilities
\u6211\u7684\u8D44\u6599|My documents
\u6211\u7684\u8BB0\u5FC6|My memory
\u8BBE\u7F6E|Settings
\u8D26\u53F7\u8BBE\u7F6E|Account settings
\u4FEE\u6539\u5BC6\u7801|Change password
\u5916\u89C2|Appearance
\u5916\u89C2\u98CE\u683C|Theme
\u660E\u4EAE|Light
\u6697\u8272|Dark
\u5207\u6362\u4E3A\u6697\u8272\u6A21\u5F0F|Switch to dark theme
\u5207\u6362\u4E3A\u660E\u4EAE\u6A21\u5F0F|Switch to light theme
\u5173\u4E8E\u6211\u4EEC|About
\u7248\u672C|Version
\u7248\u672C\u4FE1\u606F|Version information
\u79EF\u5206\u67E5\u8BE2|Credits
\u79EF\u5206\u67E5\u8BE2\u5373\u5C06\u5F00\u653E|Credits are coming soon
\u5373\u5C06\u5F00\u653E|Coming soon
\u804A\u5929\u5BA0\u7269|Chat companion
\u9009\u62E9\u804A\u5929\u5BA0\u7269|Choose chat companion
\u9009\u62E9\u804A\u5929\u4E2D\u7684\u966A\u4F34\u89D2\u8272\uFF0C\u4E5F\u53EF\u4EE5\u968F\u65F6\u5173\u95ED\u6D6E\u52A8\u5BA0\u7269\u3002|Choose a chat companion or turn off the floating companion.
\u4E86\u89E3\u5F53\u524D\u7248\u672C\u4E0E ExoHarness\u3002|About this version of ExoHarness.
ExoHarness \u662F\u8FDE\u63A5\u667A\u80FD\u52A9\u7406\u3001\u6A21\u578B\u4E0E\u5DE5\u5177\u7684\u901A\u7528\u6267\u884C\u5E73\u53F0\u3002\u901A\u8FC7\u5BF9\u8BDD\u7EC4\u7EC7\u4EFB\u52A1\uFF0C\u8BA9\u80FD\u529B\u8C03\u7528\u3001\u6267\u884C\u8FDB\u5EA6\u548C\u751F\u6210\u7ED3\u679C\u6E05\u6670\u53EF\u89C1\u3002|ExoHarness connects assistants, models and tools. Organize tasks in chat and follow tool calls, progress and results.
\u6635\u79F0\u6216\u767B\u5F55\u540D|Nickname or username
\u5F53\u524D\u5BC6\u7801|Current password
\u518D\u6B21\u8F93\u5165\u65B0\u5BC6\u7801|Re-enter new password
\u4FEE\u6539\u540E\uFF0C\u6240\u6709\u8BBE\u5907\u4E0A\u7684\u767B\u5F55\u4F1A\u8BDD\u90FD\u4F1A\u9000\u51FA\u3002|Changing your password signs you out on all devices.
\u5DF2\u5B89\u88C5\u7684\u80FD\u529B|Installed capabilities
\u53D1\u73B0\u5DF2\u6709 Skill|Discover Skills
\u641C\u7D22 Skill|Search Skills
\u4F7F\u7528\u8303\u56F4|Access scope
MCP \u6DFB\u52A0\u65B9\u5F0F|MCP input method
MCP \u5730\u5740\u3001mcpServers JSON \u6216 GitHub \u9879\u76EE|MCP URL, mcpServers JSON or GitHub project
\u7BA1\u7406\u5458\u6388\u6743\u540E\uFF0C\u53EF\u8FDE\u63A5\u8FDC\u7A0B MCP \u5730\u5740\u3001\u7C98\u8D34\u5730\u5740\u578B mcpServers JSON\uFF0C\u6216\u4ECE\u516C\u5F00 GitHub \u9879\u76EE\u5B89\u88C5\u672C\u5730 MCP\u3002Skill \u53EA\u4ECE\u516C\u5F00 GitHub \u9879\u76EE\u5B89\u88C5\uFF1B\u7528\u6237\u5B89\u88C5 Skill/MCP \u4E0D\u63A5\u53D7\u4E0A\u4F20\u6587\u4EF6\u3002\u8FDC\u7A0B MCP \u6240\u9700\u7684\u8BA4\u8BC1\u4FE1\u606F\u5728\u5BF9\u5E94\u80FD\u529B\u4E0B\u586B\u5199\uFF0C\u9875\u9762\u53EA\u663E\u793A\u201C\u5DF2\u914D\u7F6E\u201D\u3002|With administrator permission, connect a remote MCP URL, paste URL-based mcpServers JSON, or install a local MCP from public GitHub. Skills can be installed from public GitHub only. User installations do not accept uploaded files. Enter credentials in the capability settings; only configuration status is displayed.
\u5B89\u88C5 MCP|Install MCP
\u6DFB\u52A0\u4E0E\u7BA1\u7406\u8D44\u6599|Manage documents
\u5C1A\u65E0\u5206\u7C7B\u3002|No categories yet.
\u7F16\u8F91|Edit
\u518D\u70B9\u4E00\u6B21\u786E\u8BA4\u5220\u9664|Click again to confirm deletion
\u8FD8\u6CA1\u6709\u8D44\u6599\u3002\u4E0A\u4F20\u540E\uFF0C\u52A9\u7406\u53EF\u5728\u4F60\u7684\u5BF9\u8BDD\u4E2D\u68C0\u7D22\u5E76\u4F7F\u7528\u76F8\u5173\u5185\u5BB9\uFF1B\u8FD9\u91CC\u7528\u4E8E\u67E5\u770B\u548C\u7BA1\u7406\u3002|No documents yet. After uploading, your assistant can find and use relevant content in your conversations. View and manage your documents here.
\u8D44\u6599\u5DF2\u5220\u9664|Document deleted
\u8FD8\u6CA1\u6709\u8BB0\u5FC6\u3002\u53EA\u4F1A\u4FDD\u5B58\u4F60\u4E3B\u52A8\u586B\u5199\u7684\u5185\u5BB9\u3002|No memories yet. Only content you explicitly enter is saved.
\u8BB0\u5FC6\u5DF2\u5220\u9664|Memory deleted
\u52A9\u7406\u53EF\u5728\u5BF9\u8BDD\u4E2D\u68C0\u7D22\u5E76\u4F7F\u7528\u76F8\u5173\u8D44\u6599\u3002|Your assistant can find and use relevant documents during chat.
\u7BA1\u7406\u4F60\u4E3B\u52A8\u786E\u8BA4\u7684\u957F\u671F\u504F\u597D\u3002|Manage long-term preferences you explicitly confirm.
\u4FDD\u62A4\u4F60\u7684\u8D26\u53F7\u5B89\u5168\u3002|Protect your account.
\u7BA1\u7406\u4F60\u7684\u663E\u793A\u540D\u79F0\u3002|Manage your display name.
\u67E5\u770B\u5B89\u88C5\u8FDB\u5EA6\uFF0C\u7BA1\u7406\u81EA\u5DF1\u7684 MCP \u8BA4\u8BC1\u4FE1\u606F\u3002|View installation progress and manage your MCP credentials.
\u5206\u7C7B\u5DF2\u4FDD\u5B58|Category saved
\u8D44\u6599\u5DF2\u4FDD\u5B58|Document saved
\u6BCF\u6B21\u6700\u591A\u5BFC\u5165 10 \u4E2A\u6587\u4EF6\u3002|Import up to 10 files at a time.
\u6CA1\u6709\u627E\u5230\u5339\u914D\u8D44\u6599\u3002|No matching documents found.
\u8BB0\u5FC6\u5DF2\u4FDD\u5B58|Memory saved
\u4E2A\u4EBA\u8D44\u6599\u5DF2\u4FDD\u5B58|Profile saved
\u4E24\u6B21\u8F93\u5165\u7684\u65B0\u5BC6\u7801\u4E0D\u4E00\u81F4|The new passwords do not match
\u5BC6\u7801\u5DF2\u66F4\u65B0\uFF0C\u8BF7\u91CD\u65B0\u767B\u5F55|Password updated. Please sign in again.
\u8BF7\u6C42\u5931\u8D25|Request failed
\u5DF2\u5173\u95ED|Disabled
\u68C0\u67E5\u4E2D|Checking
\u72B6\u6001\u6682\u4E0D\u53EF\u7528|Status currently unavailable
\u91CD\u65B0\u68C0\u67E5|Check again
Authorization \u5DF2\u914D\u7F6E \xB7 \u66F4\u65B0|Authorization configured \xB7 Update
\u586B\u5199 Authorization|Enter Authorization
Authorization \u503C|Authorization value
\u586B\u5199\u670D\u52A1\u63D0\u4F9B\u7684\u5B8C\u6574 Header \u503C|Enter the complete header value provided by the service
\u4FDD\u5B58\u5E76\u91CD\u65B0\u68C0\u67E5|Save and check again
\u8BA4\u8BC1\u4FE1\u606F\u5DF2\u4FDD\u5B58\uFF0C\u6B63\u5728\u91CD\u65B0\u68C0\u67E5\u8FDE\u63A5\u3002|Credentials saved. Checking the connection again.
\u5220\u9664\u8BA4\u8BC1\u4FE1\u606F|Delete credentials
\u8BA4\u8BC1\u4FE1\u606F\u5DF2\u5220\u9664\uFF0CMCP \u5DF2\u505C\u7528\uFF1B\u5982\u670D\u52A1\u652F\u6301\u514D\u8BA4\u8BC1\uFF0C\u53EF\u91CD\u65B0\u68C0\u67E5\u8FDE\u63A5\u3002|Credentials deleted and MCP disabled. If the service supports access without authentication, check the connection again.
\u5DF2\u914D\u7F6E\uFF0C\u53EF\u66F4\u65B0|Configured; can be updated
\u5F85\u914D\u7F6E|Not configured
\u4EC5\u5728\u6B64\u5904\u586B\u5199\uFF0C\u4E0D\u8981\u53D1\u9001\u5230\u5BF9\u8BDD|Enter credentials here only, not in chat
\u6B63\u5728\u68C0\u67E5 MCP\uFF0C\u5B8C\u6210\u540E\u5373\u53EF\u586B\u5199\u5BC6\u94A5|Checking MCP. Enter credentials after the check completes.
\u516C\u5F00 GitHub \u6765\u6E90 \xB7 \u56FA\u5B9A\u63D0\u4EA4\u5B89\u88C5|Public GitHub source \xB7 Installed from a pinned commit
\u5B89\u88C5\u4E2D|Installing
\u53D6\u6D88\u4E0B\u8F7D|Cancel download
\u5F53\u524D\u53EF\u67E5\u770B\u5DF2\u767B\u8BB0\u7684\u4E2A\u4EBA\u80FD\u529B\uFF1B\u65B0\u7248\u81EA\u52A9\u5B89\u88C5\u529F\u80FD\u5C1A\u672A\u63A5\u5165\uFF0C\u4E0D\u80FD\u5728\u6B64\u63D0\u4EA4\u5B89\u88C5\u3002|View your registered capabilities here. Self-service installation is not connected in this version.
\u7BA1\u7406\u5458\u5C1A\u672A\u5F00\u653E\u80FD\u529B\u5B89\u88C5\u6743\u9650\uFF1B\u5DF2\u5B89\u88C5\u80FD\u529B\u4ECD\u6309\u4F7F\u7528\u6743\u9650\u548C\u52A9\u7406\u6388\u6743\u68C0\u67E5\u3002|Your administrator has not enabled capability installation. Installed capabilities still follow user and assistant permissions.
\u5C1A\u672A\u5B89\u88C5\u79C1\u6709\u80FD\u529B\u3002|No private capabilities installed yet.
\u5B89\u88C5\u4EFB\u52A1|Installation tasks
MCP \u5DF2\u767B\u8BB0\uFF0C\u6B63\u5728\u68C0\u67E5\u6765\u6E90\u3001\u8FD0\u884C\u73AF\u5883\u4E0E\u5DE5\u5177\uFF1B\u68C0\u67E5\u901A\u8FC7\u540E\u624D\u4F1A\u542F\u7528\u3002|MCP registered. Its source, runtime and tools are being checked. It will be enabled after the checks pass.
\u52A9\u7406\u53EF\u5728\u4F60\u7684\u5BF9\u8BDD\u4E2D\u68C0\u7D22\u5E76\u4F7F\u7528\u76F8\u5173\u8D44\u6599\uFF1B\u8D44\u6599\u53EA\u5C5E\u4E8E\u5F53\u524D\u8D26\u53F7\uFF0C\u4E0D\u4F1A\u81EA\u52A8\u5206\u4EAB\u7ED9\u5176\u4ED6\u7528\u6237\u3002|Your assistant can retrieve relevant documents during chat. Documents are private to your account.
\u7BA1\u7406\u6211\u7684\u8D44\u6599|Manage my documents
\u68C0\u7D22\u6211\u7684\u8D44\u6599|Search my documents
\u957F\u671F\u504F\u597D|Long-term preferences
\u4EC5\u4FDD\u5B58\u4F60\u660E\u786E\u586B\u5199\u7684\u957F\u671F\u504F\u597D\u3002\u53EF\u968F\u65F6\u505C\u7528\u3001\u4FEE\u6539\u6216\u5220\u9664\uFF1B\u4E0D\u4F1A\u81EA\u52A8\u4ECE\u804A\u5929\u4E2D\u63D0\u53D6\u3002|Save preferences you explicitly enter. Disable, edit or delete them at any time. Preferences are not extracted automatically from chat.
\u5E0C\u671B\u8BB0\u4F4F\u7684\u504F\u597D|Preferences to remember
\u542F\u7528\u8FD9\u6761\u8BB0\u5FC6|Enable this memory
\u4FDD\u5B58\u8BB0\u5FC6|Save memory
\u8FD4\u56DE\u5BF9\u8BDD|Back to chat
\u6A21\u578B\u8BF7\u6C42\u4E2D|Waiting for model
\u6B63\u5728\u6DF1\u5EA6\u601D\u8003...|Thinking deeply...
\u6B63\u5728\u6DF1\u5EA6\u601D\u8003\u2026|Thinking deeply\u2026
\u6B63\u5728\u601D\u8003...|Thinking...
\u6B63\u5728\u601D\u8003\u2026|Thinking\u2026
\u6B63\u5728\u56DE\u7B54...|Responding...
\u6B63\u5728\u751F\u6210...|Generating...
\u6B63\u5728\u751F\u6210\u2026|Generating\u2026
\u6B63\u5728\u8C03\u7528\u80FD\u529B|Calling tool
\u6B63\u5728\u8FDE\u63A5|Connecting
\u6B63\u5728\u8FDE\u63A5\u670D\u52A1|Connecting to service
\u670D\u52A1\u5728\u7EBF|Service online
\u670D\u52A1\u4E0D\u53EF\u7528|Service unavailable
\u6B63\u5728\u8BFB\u53D6|Loading
\u6B63\u5728\u8BFB\u53D6\u2026|Loading\u2026
\u6B63\u5728\u8BFB\u53D6 Skill\u2026|Loading Skill\u2026
\u6B63\u5728\u8BFB\u53D6\u5BF9\u8BDD\u2026|Loading conversations\u2026
\u6B63\u5728\u8BFB\u53D6\u7528\u6237\u2026|Loading users\u2026
\u6B63\u5728\u8BFB\u53D6\u80FD\u529B\u2026|Loading capabilities\u2026
\u6B63\u5728\u8BFB\u53D6\u8FD0\u884C\u65E5\u5FD7\u2026|Loading run logs\u2026
\u6B63\u5728\u8BFB\u53D6\u8FD0\u884C\u8F68\u8FF9|Loading trace
\u6B63\u5728\u8BFB\u53D6\u64CD\u4F5C\u8BB0\u5F55|Loading audit log
\u6B63\u5728\u8BFB\u53D6\u6743\u9650\u72B6\u6001\u3002|Loading permissions.
\u6B63\u5728\u8BFB\u53D6\u90AE\u4EF6\u670D\u52A1|Loading email settings
\u6B63\u5728\u8BFB\u53D6\u914D\u7F6E|Loading configuration
\u6B63\u5728\u6253\u5F00\u8BBE\u7F6E\u2026|Opening settings\u2026
\u6B63\u5728\u68C0\u6D4B\u8BED\u97F3\u670D\u52A1\u2026|Checking speech service\u2026
\u6B63\u5728\u68C0\u67E5|Checking
\u6B63\u5728\u68C0\u67E5\u6C99\u76D2\u4F9D\u8D56\u3002|Checking sandbox dependencies.
\u6B63\u5728\u68C0\u67E5\u6C99\u76D2\u4F9D\u8D56|Checking sandbox dependencies
\u68C0\u67E5\u6C99\u76D2\u4F9D\u8D56|Check sandbox dependencies
\u68C0\u67E5\u4E0D\u4F1A\u6267\u884C\u4E0A\u4F20\u811A\u672C\uFF1B\u901A\u8FC7\u68C0\u67E5\u4E0D\u7B49\u4E8E\u83B7\u5F97\u811A\u672C\u6267\u884C\u6388\u6743\u3002\u7F3A\u5C11\u4F9D\u8D56\u4F1A\u660E\u786E\u5217\u51FA\uFF0C\u4E0D\u56DE\u9000\u5BBF\u4E3B\u673A\u6267\u884C\u3002|Validation does not execute uploaded scripts or grant execution permission. Missing dependencies are listed. Execution remains isolated.
\u68C0\u67E5\u5DF2\u767B\u8BB0\u80FD\u529B\u548C\u52A9\u7406\u5141\u8BB8\u8303\u56F4\uFF0C\u4E0D\u81EA\u52A8\u6267\u884C\u670D\u52A1\u6216\u5B89\u88C5\u4F9D\u8D56\u3002|Check registered capabilities and assistant permissions without executing services or installing dependencies.
\u6C99\u76D2\u8FD0\u884C\u73AF\u5883|Sandbox runtime
\u6C99\u76D2\u955C\u50CF|Sandbox image
\u672C\u5730\u5DE5\u5177|Local tools
\u72EC\u7ACB\u5DE5\u5177|Independent tools
\u7B49\u5F85\u6A21\u578B\u8C03\u7528|Waiting for model request
\u6267\u884C\u8D44\u6E90|Execution resources
\u8C03\u7528|Call
\u67E5\u770B\u6280\u672F\u8BE6\u60C5|View technical details
\u67E5\u770B\u8F93\u5165\u5185\u5BB9|View input
\u654F\u611F\u5B57\u6BB5\u5DF2\u7531\u670D\u52A1\u7AEF\u8131\u654F|Sensitive fields are redacted by the server
\u9009\u62E9\u4E00\u4E2A\u4F1A\u8BDD|Select a conversation
\u540D\u79F0|Name
\u8BF4\u660E|Description
\u4E0A\u4E00\u4E2A|Previous
\u4E0B\u4E00\u4E2A|Next
\u4E0A\u4E00\u9875|Previous page
\u4E0B\u4E00\u9875|Next page
\u67E5\u8BE2 / \u5237\u65B0|Query / Refresh
\u9009\u62E9\u672C\u5730\u9879\u76EE|Select local project
\u53D8\u66F4\u8303\u56F4|Change scope
\u8BD5\u7528\u5DE5\u5177|Try tool
\u63A8\u8350|Recommended
\u4FDD\u5B58\u540E\u4E0D\u4F1A\u56DE\u663E\u660E\u6587|Plaintext is not displayed after saving
\u8F93\u5165\u540E\u5B89\u5168\u4FDD\u5B58\uFF0C\u4E0D\u4F1A\u518D\u6B21\u663E\u793A|Stored securely and not displayed again
\u641C\u7D22|Search
\u641C\u7D22\u80FD\u529B|Search capabilities
\u641C\u7D22\u4E2D\u2026|Searching\u2026
\u6B63\u5728\u641C\u7D22\u2026|Searching\u2026
\u6CA1\u6709\u627E\u5230\u76F8\u5173\u5BF9\u8BDD|No matching conversations
\u53D1\u9001\u7B2C\u4E00\u6761\u6D88\u606F\u540E\uFF0C\u4F1A\u8BDD\u4F1A\u4FDD\u5B58\u5728\u8FD9\u91CC\u3002|Conversations appear here after your first message.
\u7F6E\u9876|Pin
\u53D6\u6D88\u7F6E\u9876|Unpin
\u91CD\u547D\u540D|Rename
\u5220\u9664|Delete
\u590D\u5236|Copy
\u5DF2\u590D\u5236|Copied
\u70B9\u8D5E|Like
\u8E29|Dislike
\u91CD\u65B0\u751F\u6210|Regenerate
\u7EE7\u7EED\u8FD0\u884C|Resume
\u7EE7\u7EED\u6267\u884C|Resume
\u7B49\u5F85\u6388\u6743|Awaiting authorization
\u53EF\u6062\u590D|Recoverable
\u6392\u961F\u4E2D|Queued
\u6B63\u5728\u6267\u884C|Running
\u8FD0\u884C\u4E2D|Running
\u51C6\u5907\u4E2D|Preparing
\u5DF2\u5B8C\u6210|Completed
\u5B8C\u6210|Completed
\u6210\u529F|Succeeded
\u5931\u8D25|Failed
\u5DF2\u5931\u8D25|Failed
\u5DF2\u53D6\u6D88|Cancelled
\u53D6\u6D88|Cancel
\u5DF2\u505C\u6B62|Stopped
\u7B49\u5F85\u4E2D|Waiting
\u672A\u5F00\u59CB|Not started
\u95F2\u7F6E|Idle
\u7A7A\u95F2|Idle
\u6B63\u5728\u5F55\u97F3|Recording
\u5F00\u59CB\u5F55\u97F3|Start recording
\u505C\u6B62\u5F55\u97F3|Stop recording
\u8BED\u97F3\u8F93\u5165|Voice input
\u4E0B\u8F7D\u56FE\u7247|Download image
\u4E0B\u8F7D\u89C6\u9891|Download video
\u4E0B\u8F7D\u97F3\u9891|Download audio
\u4E0B\u8F7D\u6587\u4EF6|Download file
\u4E0B\u8F7D|Download
\u9884\u89C8|Preview
\u6253\u5F00\u9884\u89C8|Open preview
\u4ECA\u5929|Today
\u6628\u5929|Yesterday
\u8FC7\u53BB\u4E09\u5929|Last 3 days
\u66F4\u65E9|Earlier
\u66F4\u591A|More
\u663E\u793A\u66F4\u591A|Show more
\u672A\u547D\u540D\u4F1A\u8BDD|Untitled chat
\u65B0\u5BF9\u8BDD\u540D\u79F0\uFF0C\u6309\u56DE\u8F66\u4FDD\u5B58|New chat name; press Enter to save
\u8BF7\u5148\u505C\u6B62\u6B63\u5728\u8FD0\u884C\u7684\u5BF9\u8BDD|Stop the active run first
\u7F16\u7801|Coding
\u6B63\u5728\u4FDD\u5B58\u2026|Saving\u2026
\u4FDD\u5B58|Save
\u5173\u95ED|Close
\u8BFB\u53D6\u5931\u8D25|Loading failed
\u65E0\u6570\u636E|No data
\u6682\u65F6\u6CA1\u6709\u6570\u636E|No data yet
\u6682\u65F6\u6CA1\u6709\u8BB0\u5F55|No records yet
\u6682\u65E0\u8BB0\u5F55|No records yet
\u6682\u65E0\u6570\u636E|No data yet
\u6682\u672A\u914D\u7F6E|Not configured
\u5DF2\u914D\u7F6E|Configured
\u4E0D\u53EF\u7528|Unavailable
\u53EF\u7528|Available
\u672A\u5C31\u7EEA|Not ready
\u5DF2\u5C31\u7EEA|Ready
\u672A\u542F\u7528|Disabled
\u66F4\u65B0\u68C0\u67E5|Update check
\u4FDD\u5B58\u4E2D|Saving
\u641C\u7D22\u7ED3\u679C|Search results
\u5168\u90E8\u5DF2\u9009|All selected
\u6E05\u9664|Clear
\u590D\u5236\u6210\u529F|Copied
\u663E\u793A|Show
\u9690\u85CF|Hide
\u5173\u95ED\u5BA0\u7269|Disable companion
\u542F\u7528\u5BA0\u7269|Enable companion
\u8FD4\u56DE|Back
\u9009\u62E9\u6587\u4EF6|Choose file
\u672A\u9009\u62E9|Not selected
\u6253\u5F00|Open
\u8FDE\u63A5|Connect
\u5DE5\u5177|Tool
\u9700\u8981\u767B\u5F55|Sign-in required
\u8EAB\u4EFD|Identity
\u6587\u6863|Document
\u97F3\u9891|Audio
\u56FE\u7247|Image
\u89C6\u9891|Video
\u8FD0\u884C|Run
\u8BE6\u7EC6\u4FE1\u606F|Details
\u5DF2\u4FDD\u5B58|Saved
\u9ED8\u8BA4|Default
\u65B0\u7528\u6237|New user
\u641C\u7D22\u6280\u80FD|Search Skills
\u5168\u9009|Select all
\u53D6\u6D88\u5168\u9009|Clear selection
0 \u4E2A\u6A21\u578B|0 models
0 \u4E2A\u52A9\u7406|0 assistants
\u4F8B\u5982\uFF1A\u4E3B\u529B\u6A21\u578B|Example: primary model
\u4F8B\u5982\uFF1A\u538B\u7F29\u6307\u5B9A\u56FE\u7247|Example: compress selected images
\u4F8B\u5982\uFF1A\u54C1\u724C, \u95E8\u5E97|Example: brand, store
\u4F8B\u5982\uFF1A\u54C1\u724C\u89C4\u8303|Example: brand guidelines
\u4F8B\u5982\uFF1A\u5904\u7406\u9879\u76EE\u4E2D\u7684\u56FE\u7247\u6587\u4EF6|Example: process project images
\u4F8B\u5982\uFF1A\u67E5\u8BE2\u4E1C\u4EAC\u672A\u6765\u4E09\u5929\u7684\u5929\u6C14|Example: weather in Tokyo for the next three days
\u4F8B\u5982\uFF1A\u67E5\u8BE2\u516C\u53F8\u5185\u90E8\u77E5\u8BC6\u5E93|Example: search the company knowledge base
\u4F8B\u5982\uFF1A\u8BFB\u53D6\u672C\u5730\u9879\u76EE\u6570\u636E|Example: read local project data
\u4F8B\u5982 \u5C0F\u6F9C|Example: Xiaolan
\u8F93\u5165\u8981\u67E5\u627E\u7684\u8BCD\u53E5|Enter search terms
\u914D\u7F6E\u52A9\u7406|Configure assistant
\u914D\u7F6E|Configure
\u4E86\u89E3\u5F53\u524D\u7248\u672C|About this version
\u6CE8\u518C|Register
\u6CE8\u518C\u7528\u6237|Registered users
\u641C\u7D22\u5386\u53F2|Search history
ExoHarness \u6280\u80FD\u4E2D\u5FC3|ExoHarness capabilities
ExoHarness \u8FD0\u884C\u603B\u89C8|ExoHarness overview
ExoHarness \u5BF9\u8BDD|ExoHarness chat
\u667A\u80FD\u52A9\u7406\u914D\u7F6E|Assistant configuration
\u8F93\u5165|Input
\u8F93\u51FA|Output
\u4E8B\u4EF6|Events
\u7ED3\u679C|Result
\u9519\u8BEF|Error
\u72B6\u6001|Status
\u6A21\u578B\u54CD\u5E94|Model response
\u5DE5\u5177\u8C03\u7528|Tool call
\u5DE5\u5177\u8FD4\u56DE|Tool result
\u767B\u5F55\u5931\u8D25|Sign-in failed
\u6B63\u5728\u767B\u5F55\u2026|Signing in\u2026
\u6B63\u5728\u767B\u5F55|Signing in
\u767B\u5F55\u4E2D\u2026|Signing in\u2026
\u6B63\u5728\u6CE8\u518C\u2026|Registering\u2026
\u9A8C\u8BC1\u7801\u5DF2\u53D1\u9001|Verification code sent
\u5BC6\u7801\u81F3\u5C11 8 \u4F4D|Password must be at least 8 characters
\u8BF7\u586B\u5199\u90AE\u7BB1\u548C\u5BC6\u7801|Enter your email and password
\u8BF7\u586B\u5199\u5BC6\u7801|Enter your password
\u8BF7\u586B\u5199\u90AE\u7BB1|Enter your email
\u4E24\u6B21\u8F93\u5165\u7684\u5BC6\u7801\u4E0D\u4E00\u81F4|Passwords do not match
\u90AE\u7BB1\u3001\u7528\u6237\u540D\u6216\u5BC6\u7801\u4E0D\u6B63\u786E|Incorrect email, username or password
\u767B\u5F55\u5C1D\u8BD5\u8FC7\u591A\uFF0C\u8BF7\u7A0D\u540E\u518D\u8BD5|Too many sign-in attempts. Try again later.
\u8BED\u97F3\u670D\u52A1\u4E0D\u53EF\u7528|Speech service unavailable
\u8BED\u97F3\u670D\u52A1\u53EF\u7528|Speech service available
\u8FDE\u63A5\u5931\u8D25|Connection failed
\u53D6\u6D88\u8BF7\u6C42\u5DF2\u53D1\u9001|Cancellation requested
\u672C\u6B21\u8FD0\u884C\u5931\u8D25|Run failed
\u4EFB\u52A1\u5B8C\u6210|Task completed
\u4EFB\u52A1\u5931\u8D25|Task failed
\u4EFB\u52A1\u5DF2\u53D6\u6D88|Task cancelled
\u6267\u884C\u5DF2\u505C\u6B62|Execution stopped
\u9700\u8981\u6388\u6743|Authorization required
\u6A21\u578B\u54CD\u5E94\u672A\u5B8C\u6210|Incomplete model response
\u8FDE\u63A5\u6B63\u5E38|Connected
\u5DF2\u8FDE\u63A5|Connected
\u672A\u8FDE\u63A5|Disconnected
\u7B49\u5F85|Waiting
\u52A0\u8F7D\u4E2D|Loading
\u7528\u6237\u7AEF|User portal
\u7BA1\u7406\u7AEF|Management console
\u5B89\u5168|Security
\u8D44\u6599|Documents
\u80FD\u529B\u68C0\u67E5|Capability checks
\u8FD4\u56DE\u9996\u9875|Back to home
\u63D0\u4EA4|Submit
\u6D3B\u52A8\u4EFB\u52A1|Active tasks
\u6388\u6743|Authorize
\u62D2\u7EDD|Deny
\u5141\u8BB8|Allow
\u6D88\u606F|Message
\u65E0|None
\u672A\u77E5|Unknown
\u8BF7\u7A0D\u5019|Please wait
\u6309\u56DE\u8F66\u53D1\u9001|Press Enter to send
\u5173\u95ED\u83DC\u5355|Close menu
\u6253\u5F00\u83DC\u5355|Open menu
`.trim().split("\n").map((line) => line.split("|"));
  var companionCatalog = new Map(`
\u56E2\u5B50|Blob companions
\u5C0F\u5973\u5B69|Girl companions
\u82BD\u82BD|Sprout
\u7130\u7130|Flame
\u58A8\u58A8|Ink
\u6CE2\u6CE2|Bubble
\u6838\u6838|Core
\u56E2\u56E2|Bun
\u5C0F\u82BD|Mint
\u5C0F\u6A59|Ember
\u5C0F\u591C|Night
\u5C0F\u6C50|Tide
\u5C0F\u82AF|Chip
\u5C0F\u68C9|Cotton
\u5934\u9876\u5C0F\u82BD\u7684\u7EFF\u56E2\u5B50|Green companion with a little sprout
\u84EC\u5C3E\u5C0F\u72D0|Fluffy-tailed fox
\u591C\u8272\u5C0F\u732B|Night-colored cat
\u4F1A\u53D1\u5149\u7684\u6C34\u6EF4|Glowing water droplet
\u5E26\u5929\u7EBF\u7684\u5C0F\u673A\u5668\u4EBA|Little robot with an antenna
\u5782\u8033\u5C0F\u5154|Floppy-eared bunny
\u8584\u8377\u7EFF\u77ED\u53D1\uFF0C\u5143\u6C14\u6821\u670D|Short mint hair and a lively school uniform
\u6A59\u53D1\u72D0\u8033\uFF0C\u6D3B\u6CFC\u6821\u670D|Orange hair, fox ears and a lively school uniform
\u7D2B\u53D1\u732B\u8033\uFF0C\u5B89\u9759\u6821\u670D|Purple hair, cat ears and a quiet school uniform
\u84DD\u53D1\u6C34\u6EF4\uFF0C\u6E05\u723D\u6821\u670D|Blue hair, droplet accent and a fresh school uniform
\u94F6\u53D1\u661F\u6807\uFF0C\u79D1\u6280\u6821\u670D|Silver hair, star accent and a tech-inspired school uniform
\u7C89\u53D1\u5154\u5939\uFF0C\u751C\u7F8E\u6821\u670D|Pink hair, bunny clip and a sweet school uniform
`.trim().split("\n").map((line) => line.split("|")));
  var catalog = new Map(entries);
  function english(source) {
    const direct = catalog.get(source);
    if (direct !== void 0) return direct;
    const patterns = [
      [/^更新于 (.+)$/, (time) => `Updated at ${time}`],
      [/^上次保存：(.+)$/, (time) => `Last saved: ${time}`],
      [/^(用户筛选|筛选)：(.+)$/, (kind, value) => `${kind === "\u7528\u6237\u7B5B\u9009" ? "User filter" : "Filter"}: ${value}`],
      [/^第 (\d+) 轮 · 第 (\d+) 次执行$/, (turn, attempt) => `Turn ${turn} \xB7 Attempt ${attempt}`],
      [/^第 (\d+) 轮(?: · (本轮执行|重试 (\d+)))?$/, (turn, suffix, retry) => `Turn ${turn}${suffix ? retry ? ` \xB7 Retry ${retry}` : " \xB7 Current run" : ""}`],
      [/^(\d+) 轮消息 · ([\d,]+) Token$/, (turns, tokens) => `${turns} turns \xB7 ${tokens} tokens`],
      [/^(\d+) 轮消息 · (\d+) 次执行$/, (turns, runs) => `${turns} turns \xB7 ${runs} runs`],
      [/^(\d+) 次模型请求 · (\d+) 次能力调用$/, (requests, calls) => `${requests} model requests \xB7 ${calls} capability calls`],
      [/^(\d+) 次请求 · (\d+) 次重试 · ([\d,]+) Token$/, (requests, retries, tokens) => `${requests} requests \xB7 ${retries} retries \xB7 ${tokens} tokens`],
      [/^(\d+) 次模型请求$/, (count2) => `${count2} model requests`],
      [/^(\d+) 次(能力调用|请求|重试)$/, (count2, kind) => `${count2} ${{ "\u80FD\u529B\u8C03\u7528": "capability calls", "\u8BF7\u6C42": "requests", "\u91CD\u8BD5": "retries" }[kind]}`],
      [/^相比上次请求：新增 (\d+) 项，移除 (\d+) 项，复用 (\d+) 项。(逻辑输入可核验。|含脱敏或省略内容。) 复用指日志存储去重，不代表模型没有接收或消耗 Token。(?: 已记录 (\d+) 个上下文变换，可核验变换前后的来源。)?$/, (added, removed, retained, exact, changes) => `Compared with the previous request: ${added} added, ${removed} removed, ${retained} retained. ${exact === "\u903B\u8F91\u8F93\u5165\u53EF\u6838\u9A8C\u3002" ? "Logical input is verifiable." : "Includes redacted or omitted content."} Retention refers to journal deduplication, not avoided model input or tokens.${changes ? ` ${changes} context transformations recorded with verifiable sources.` : ""}`],
      [/^(\d+) 次请求(未提供用量|仅提供部分用量|尚无结束记录)$/, (count2, kind) => `${count2} requests ${{ "\u672A\u63D0\u4F9B\u7528\u91CF": "without usage", "\u4EC5\u63D0\u4F9B\u90E8\u5206\u7528\u91CF": "with partial usage", "\u5C1A\u65E0\u7ED3\u675F\u8BB0\u5F55": "without completion records" }[kind]}`],
      [/^含历史累计 ([\d,]+) Token$/, (tokens) => `Includes ${tokens} historical tokens`],
      [/^变换 (\d+)$/, (sequence) => `Transformation ${sequence}`],
      [/^原 (\S+) (\d+)$/, (kind, index) => `Original ${kind} ${index}`],
      [/^([\d,]+) 字节$/, (bytes) => `${bytes} bytes`],
      [/^(模型配置|系统说明|消息|工具 Schema) (\d+)$/, (label, index) => `${english(label)} ${index}`],
      [/^运行(成功|失败|已取消|可恢复|运行中)$/, (status) => `Run: ${english(status)}`],
      [/^(\d+) 个文字片段已合并$/, (count2) => `${count2} text fragments grouped`],
      [/^查看 (\d+) 条原始事件$/, (count2) => `View ${count2} raw events`],
      [/^(\d+) 条展示 · (\d+) 条原始事件(?: · 当前事件 (#.+) \/ 最新 #(\d+))?$/, (shown, raw, range, latest) => `${shown} displayed \xB7 ${raw} raw events${range ? ` \xB7 Current events ${range} / Latest #${latest}` : ""}`],
      [/^(调用|模型请求) (\d+) \/ (\d+)$/, (kind, index, total) => `${kind === "\u8C03\u7528" ? "Invocation" : "Model request"} ${index} / ${total}`],
      [/^(读取|写入) (.+)$/, (action2, value) => `${action2 === "\u8BFB\u53D6" ? "Read" : "Write"} ${english(value)}`],
      [/^(\d+) 个依赖连接 · (\d+) 个已授权服务$/, (bindings, services) => `${bindings} dependency connections \xB7 ${services} authorized services`],
      [/^(\d+) 轮消息 · 更新于 (.+)$/, (turns, time) => `${turns} turns \xB7 Updated at ${time}`],
      [/^(\d+) 项能力 · (\d+) 项未就绪$/, (total, blocked) => `${total} capabilities \xB7 ${blocked} not ready`],
      [/^(\d+) 项能力$/, (count2) => `${count2} capabilities`],
      [/^共 (\d+) 项$/, (count2) => `${count2} items`],
      [/^共 (\d+) 条记录$/, (count2) => `${count2} records`],
      [/^显示最近 (\d+) 条记录$/, (count2) => `Showing the latest ${count2} records`],
      [/^显示 (\d+) 条 · 共 (\d+) 条$/, (shown, total) => `Showing ${shown} of ${total} records`],
      [/^显示 (\d+) 项 · 共 (\d+) 项$/, (shown, total) => `Showing ${shown} of ${total} items`],
      [/^(\d+) 个配置$/, (count2) => `${count2} configurations`],
      [/^(\d+) 个会话$/, (count2) => `${count2} chats`],
      [/^(\d+) 个$/, (count2) => `${count2} total`],
      [/^第 (\d+) 版$/, (revision) => `Revision ${revision}`],
      [/^(Skill|MCP 服务|Tool) · (v[^ ·]+) · 第 (\d+) 版$/, (kind, version, revision) => `${kind === "MCP \u670D\u52A1" ? "MCP service" : kind} \xB7 ${version} \xB7 Revision ${revision}`],
      [/^(\d+) (分钟|秒)$/, (value, unit) => `${value} ${unit === "\u5206\u949F" ? "min" : "sec"}`],
      [/^(\d+) 次未上报$/, (count2) => `${count2} unreported`],
      [/^每用户最多 (\d+) 个活跃 Run$/, (count2) => `Up to ${count2} active runs per user`],
      [/^(\d+)\/(\d+) 已授权$/, (allowed, total) => `${allowed}/${total} authorized`],
      [/^(助理|配置名称|协议|接口|Profile|音色|语言|项目)：(.+)$/, (label, value) => `${{ "\u52A9\u7406": "Assistant", "\u914D\u7F6E\u540D\u79F0": "Configuration", "\u534F\u8BAE": "Protocol", "\u63A5\u53E3": "Endpoint", "Profile": "Profile", "\u97F3\u8272": "Voice", "\u8BED\u8A00": "Language", "\u9879\u76EE": "Project" }[label]}: ${value}`],
      [/^(修改智能助理|更新当前助理) · (.+)$/, (action2, name) => `${action2 === "\u4FEE\u6539\u667A\u80FD\u52A9\u7406" ? "Edit assistant" : "Update assistant"} \xB7 ${name}`],
      [/^上下文 ([\d,]+) · (最大输出 ([\d,]+) tokens|输出长度由模型服务决定)$/, (context, output, max) => `Context ${context} \xB7 ${max ? `Maximum output ${max} tokens` : english(output)}`],
      [/^(.+)：执行连接或内容尚未就绪$/, (name) => `${name}: Execution connection or content not ready`],
      [/^(\d+) 项可用性提醒 · 展开查看$/, (count2) => `${count2} availability notices \xB7 Expand to view`]
    ];
    for (const [pattern, format] of patterns) {
      const match = source.match(pattern);
      if (match) return format(...match.slice(1));
    }
    if (source.includes("\n")) return source.split("\n").map(english).join("\n");
    if (source.includes(" \xB7 ")) return source.split(" \xB7 ").map(english).join(" \xB7 ");
    if (source.includes(" \u2192 ")) return source.split(" \u2192 ").map(english).join(" \u2192 ");
    if (source.includes(" / ")) return source.split(" / ").map(english).join(" / ");
    const title = source.match(/^(.+) · ExoHarness$/);
    if (title && catalog.has(title[1])) return `${catalog.get(title[1])} \xB7 ExoHarness`;
    const decorated = source.match(/^([←＋：\s]*)(.+?)([→\s]*)$/);
    if (decorated && catalog.has(decorated[2])) return decorated[1] + catalog.get(decorated[2]) + decorated[3];
    const example = source.match(/^例如[：:\s]*(.+)$/);
    if (example) return `Example: ${example[1]}`;
    const count = source.match(/^(\d+)\s*个(模型|助理)$/);
    if (count) return `${count[1]} ${count[2] === "\u6A21\u578B" ? "models" : "assistants"}`;
    const action = source.match(/^(打开对话|对话操作|删除对话)[：:](.*)$/);
    if (action) return `${{ "\u6253\u5F00\u5BF9\u8BDD": "Open chat", "\u5BF9\u8BDD\u64CD\u4F5C": "Chat actions", "\u5220\u9664\u5BF9\u8BDD": "Delete chat" }[action[1]]}: ${action[2]}`;
    return source;
  }

  // src/i18n.ts
  var storageKey = "exoharness.ui-language";
  var protectedContent = [
    "script",
    "style",
    "pre",
    "code",
    "textarea",
    "input",
    "[contenteditable]",
    '[data-i18n="off"]',
    ".rich-content",
    ".tool-progress-message",
    ".session-item-heading strong",
    ".session-item-meta",
    ".user-conversation-title",
    "#conversation-title",
    "#portal-user-name",
    "#portal-user-email",
    "#trace-conversation-title",
    ".video-artifact-heading strong",
    "#skill-instructions",
    ".skill-instructions",
    ".artifact-name",
    ".user-conversation-prompt",
    ".user-identity",
    ".user-installed-item > strong",
    ".capability-card strong",
    ".secret-row-heading strong",
    ".validation-check span",
    "#detail-name",
    "#detail-description",
    "#skill-instruction-content",
    ".assistant-card strong",
    ".agent-card strong",
    ".model-card strong",
    ".overview-task strong",
    ".readiness-item:not(:first-child) > strong",
    ".user-list-name",
    ".user-list-avatar",
    ".agent-list-heading strong",
    ".agent-list-copy > span:not(.agent-list-heading)",
    ".agent-list-avatar",
    ".model-item-title strong",
    "#default-model-summary",
    "#default-tts-summary",
    "#default-asr-summary",
    ".trace-run-item-head strong",
    ".trace-run-prompt",
    ".trace-turn-prompt",
    "#trace-prompt"
  ].join(",");
  var attributes = ["placeholder", "title", "aria-label"];
  var textSources = /* @__PURE__ */ new WeakMap();
  var attributeSources = /* @__PURE__ */ new WeakMap();
  var selectors = /* @__PURE__ */ new Set();
  var language = "zh-CN";
  try {
    if (localStorage.getItem(storageKey) === "en") language = "en";
  } catch {
  }
  function render(source, companion = false) {
    if (language === "zh-CN") return source;
    const core = source.trim();
    return core ? source.replace(core, companion ? companionCatalog.get(core) || english(core) : english(core)) : source;
  }
  function translateText(node) {
    if (!node.parentElement || node.parentElement.closest(protectedContent)) return;
    const previous = textSources.get(node);
    const source = previous && node.data === previous.rendered ? previous.source : node.data;
    const rendered = render(source, Boolean(node.parentElement.closest(".chat-pet-card, .chat-pet-gallery-group > h2")));
    textSources.set(node, { source, rendered });
    if (node.data !== rendered) node.data = rendered;
  }
  function translateAttributes(element) {
    if (element.closest('[data-i18n="off"],pre,code,.rich-content,.tool-progress-message')) return;
    let sources = attributeSources.get(element);
    if (!sources) {
      sources = /* @__PURE__ */ new Map();
      attributeSources.set(element, sources);
    }
    for (const name of attributes) {
      const current = element.getAttribute(name);
      if (current === null) {
        sources.delete(name);
        continue;
      }
      if (name === "title" && element.matches(".session-item,.video-artifact-heading strong")) continue;
      const previous = sources.get(name);
      const source = previous && current === previous.rendered ? previous.source : current;
      const rendered = render(source);
      sources.set(name, { source, rendered });
      if (rendered !== current) element.setAttribute(name, rendered);
    }
  }
  function translateTree(root) {
    if (root instanceof Text) {
      translateText(root);
      return;
    }
    if (root instanceof Element && root.closest(protectedContent)) {
      if (root instanceof HTMLInputElement || root instanceof HTMLTextAreaElement) translateAttributes(root);
      return;
    }
    if (root instanceof Element) translateAttributes(root);
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        if (node instanceof Element && node.matches(protectedContent)) {
          if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) translateAttributes(node);
          return NodeFilter.FILTER_REJECT;
        }
        return NodeFilter.FILTER_ACCEPT;
      }
    });
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (node instanceof Text) translateText(node);
      else if (node instanceof Element) translateAttributes(node);
    }
  }
  var pending = /* @__PURE__ */ new Set();
  var scheduled = false;
  var observer = new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === "childList") for (const node of record.addedNodes) pending.add(node);
      else pending.add(record.target);
    }
    if (!scheduled && pending.size) {
      scheduled = true;
      queueMicrotask(() => {
        scheduled = false;
        observer.disconnect();
        for (const node of pending) if (node.isConnected) translateTree(node);
        pending.clear();
        observe();
      });
    }
  });
  function observe() {
    observer.observe(document.documentElement, {
      subtree: true,
      childList: true,
      characterData: true,
      attributes: true,
      attributeFilter: [...attributes]
    });
  }
  function setLanguage(next, persist = true) {
    language = next;
    if (persist) try {
      localStorage.setItem(storageKey, language);
    } catch {
    }
    observer.disconnect();
    document.documentElement.lang = language;
    translateTree(document.documentElement);
    selectors.forEach((select) => {
      select.value = language;
    });
    observe();
    document.dispatchEvent(new CustomEvent("exoharness:languagechange", { detail: { language } }));
  }
  function installSwitcher() {
    const label = document.createElement("label");
    label.className = "ui-language-switch";
    label.dataset.i18n = "off";
    label.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a18 18 0 0 1 0 18 18 18 0 0 1 0-18Z"/></svg><select aria-label="Language / \u8BED\u8A00"><option value="zh-CN">\u4E2D\u6587</option><option value="en">English</option></select>';
    const select = label.querySelector("select");
    selectors.add(select);
    select.value = language;
    select.addEventListener("change", () => setLanguage(select.value === "en" ? "en" : "zh-CN"));
    const anchor = document.querySelector(".chat-header-actions") || document.querySelector(".topbar") || document.querySelector(".portal-access-shell,.login-shell");
    (anchor || document.body).append(label);
    if (!anchor || anchor.matches(".portal-access-shell,.login-shell")) label.classList.add("ui-language-corner");
    const style = document.createElement("style");
    style.dataset.i18n = "off";
    style.textContent = `.ui-language-switch{display:inline-flex;align-items:center;gap:6px;flex-shrink:0;order:100;margin-left:auto;padding:5px 8px;border:1px solid var(--portal-line,var(--border,var(--line,#7775)));border-radius:10px;color:inherit;background:var(--portal-panel,var(--surface,#20222b));font:13px/1.4 system-ui}.ui-language-switch svg{width:17px;height:17px;flex-shrink:0}.ui-language-switch select{min-width:70px;max-width:100px;width:auto;padding:2px;border:0;background:transparent;color:inherit;font:inherit;cursor:pointer}.ui-language-switch select option{color:var(--portal-text,var(--text,#20222b));background:var(--portal-panel,var(--surface-strong,#fff))}.ui-language-switch select:focus-visible{outline:2px solid #7189ed;outline-offset:3px}.ui-language-corner{position:fixed;top:18px;right:18px;z-index:30}.ui-language-switch:not(.ui-language-corner){align-self:center}@media(max-width:600px){.ui-language-switch{gap:3px;padding:4px 5px;font-size:12px}.ui-language-switch select{min-width:62px}.ui-language-corner{top:12px;right:12px}}`;
    document.head.append(style);
  }
  var nativeConfirm = window.confirm.bind(window);
  var nativePrompt = window.prompt.bind(window);
  window.confirm = (message) => nativeConfirm(render(String(message)));
  window.prompt = (message, value) => nativePrompt(render(String(message)), value);
  window.addEventListener("storage", (event) => {
    if (event.key === storageKey) setLanguage(event.newValue === "en" ? "en" : "zh-CN", false);
  });
  function start() {
    installSwitcher();
    setLanguage(language, false);
  }
  setLanguage(language, false);
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
  else start();
})();
