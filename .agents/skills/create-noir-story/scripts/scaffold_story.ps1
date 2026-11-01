param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[a-z0-9_]+$')]
    [string]$StoryId,

    [Parameter(Mandatory = $true)]
    [string]$StoryName
)

$ErrorActionPreference = "Stop"

$SkillDir = Split-Path -Parent $PSScriptRoot
$RepoRoot = Resolve-Path (Join-Path $SkillDir "..\..\..")

$CaseOut   = Join-Path $RepoRoot "assets\cases\$StoryId.ron"
$ScenesOut = Join-Path $RepoRoot "assets\scenes\${StoryId}_scenes.ron"

if ((Test-Path $CaseOut) -or (Test-Path $ScenesOut)) {
    throw "Story files already exist for '$StoryId': check $CaseOut or $ScenesOut"
}

$CaseTpl   = Get-Content -Raw (Join-Path $SkillDir "resources\case_template.ron")
$ScenesTpl = Get-Content -Raw (Join-Path $SkillDir "resources\scenes_template.ron")

$UpperTitle = $StoryName.ToUpperInvariant()

$CaseContent   = $CaseTpl.Replace("__STORY_UPPER__", $UpperTitle).Replace("__STORY_NAME__", $StoryName)
$ScenesContent = $ScenesTpl.Replace("__STORY_NAME__", $StoryName)

Set-Content -Path $CaseOut -Value $CaseContent -Encoding UTF8
Set-Content -Path $ScenesOut -Value $ScenesContent -Encoding UTF8

$ConstIdent = $StoryId.ToUpperInvariant()

Write-Host "Scaffolded new Crimson Ledger story '$StoryName' ($StoryId):" -ForegroundColor Green
Write-Host "  1. Case RON:   assets/cases/$StoryId.ron"
Write-Host "  2. Scenes RON: assets/scenes/${StoryId}_scenes.ron"
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  - Customize scenes, props, clues, and dialogue in both .ron files."
Write-Host "  - Register StoryEntry in src/stories.rs using EMBEDDED_${ConstIdent} and EMBEDDED_${ConstIdent}_SCENES."
Write-Host "  - Run: cargo test --bin vampire_noir_rpg"
