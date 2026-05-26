# Rebuild recording JSON from the logged click coordinates
$clicks = @(
  @(363, 1023), @(625, 1342), @(724, 825), @(792, 1334), @(720, 780),
  @(947, 1289), @(765, 818), @(1170, 1371), @(751, 851), @(666, 1353)
)
# 16 reps of (730, 836)
1..16 | ForEach-Object { $clicks += ,@(730, 836) }
$clicks += ,@(832, 1268)
# 15 reps of (729, 843)
1..15 | ForEach-Object { $clicks += ,@(729, 843) }
$clicks += ,@(947, 1289)
# 23 reps of (743, 838)
1..23 | ForEach-Object { $clicks += ,@(743, 838) }
$clicks += ,@(1097, 1332)
# 15 reps of (739, 826)
1..15 | ForEach-Object { $clicks += ,@(739, 826) }
$clicks += ,@(1310, 1343)
# 9 reps of (739, 827)
1..9 | ForEach-Object { $clicks += ,@(739, 827) }
# Tail sequence
$tail = @(
  @(1495, 1286), @(856, 888), @(856, 888), @(1640, 1364), @(863, 889),
  @(1834, 1352), @(879, 898), @(1961, 1327), @(794, 882), @(2082, 1328),
  @(915, 942), @(2313, 1308), @(2270, 581), @(2403, 302), @(2463, 205),
  @(2495, 635), @(2255, 1061), @(2473, 1348), @(2191, 605), @(2624, 1364),
  @(2578, 578), @(2537, 464), @(2528, 373), @(2609, 165), @(2540, 118),
  @(2632, 333), @(2668, 577), @(2639, 690), @(2151, 1315), @(2010, 1315),
  @(1792, 1328), @(1631, 1331), @(2789, 1315), @(3152, 518)
)
foreach ($p in $tail) { $clicks += ,$p }

"Total clicks: $($clicks.Count)"

# Build actions with 0.3s spacing
$actions = @()
$ts = 0.0
foreach ($c in $clicks) {
  $actions += [PSCustomObject]@{
    type          = "click"
    x             = $c[0]
    y             = $c[1]
    timestamp     = $ts
    relative_time = $ts
  }
  $ts = [math]::Round($ts + 0.3, 3)
}

$recording = [PSCustomObject]@{
  name     = "recovered_raid"
  created  = "20260526_085812"
  duration = $ts
  actions  = $actions
}

$out = "F:\Git\coc-attack-bot-rs\recordings\recovered_raid_20260526_085812.json"
$recording | ConvertTo-Json -Depth 5 | Set-Content -Path $out -Encoding ascii
"Saved: $out"
"Duration: ${ts}s, actions: $($actions.Count)"

# Verify it loads as the bot expects
"`n=== First 3 actions in file ==="
$check = Get-Content $out | ConvertFrom-Json
$check.actions[0..2] | Format-List
"`n=== File size ==="
(Get-Item $out).Length
