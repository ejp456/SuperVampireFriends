#!/usr/bin/env bash
# ==============================================================================
# One-time setup: Keyless GitHub Actions -> Google Cloud Compute Engine (WIF)
# Configures Workload Identity Federation locked to ejp456/SuperVampireFriends
# so GitHub Actions can push vampire_noir_rpg to the streaming-friends VM.
# ==============================================================================
set -euo pipefail

GITHUB_REPO="${GITHUB_REPO:-ejp456/SuperVampireFriends}"
VM_NAME="${VM_NAME:-instance-20261031-205001}"
DEPLOY_DIR="${DEPLOY_DIR:-/opt/crimson-ledger}"
POOL_NAME="${POOL_NAME:-github-actions-pool}"
PROVIDER_NAME="${PROVIDER_NAME:-github-oidc}"
SA_NAME="${SA_NAME:-github-svf-deployer}"

if ! command -v gcloud >/dev/null 2>&1 && [ -x "$HOME/google-cloud-sdk/bin/gcloud" ]; then
  export PATH="$HOME/google-cloud-sdk/bin:$PATH"
fi

if ! command -v gcloud >/dev/null 2>&1; then
  echo "Error: gcloud CLI is not installed or not in PATH." >&2
  exit 1
fi

PROJECT_ID="${GCP_PROJECT_ID:-streaming-friends-server}"

echo "==> Using Google Cloud project: $PROJECT_ID"
PROJECT_NUMBER="$(gcloud projects describe "$PROJECT_ID" --format='value(projectNumber)')"

echo "==> Enabling required Google Cloud APIs (IAM, STS, Compute)..."
gcloud services enable \
  iam.googleapis.com \
  iamcredentials.googleapis.com \
  sts.googleapis.com \
  compute.googleapis.com \
  --project="$PROJECT_ID" \
  --quiet

ZONE="${GCP_ZONE:-$(gcloud compute instances list --project="$PROJECT_ID" --filter="name:${VM_NAME}" --limit=1 --format='value(zone.basename())' --quiet || true)}"
if [ -z "$ZONE" ]; then
  echo "Warning: Could not auto-detect zone for VM '${VM_NAME}' in project '${PROJECT_ID}'."
  echo "         Set GCP_ZONE=<zone> (e.g. us-central1-a) when configuring GitHub variables."
else
  echo "==> Detected VM '${VM_NAME}' in zone: ${ZONE}"
fi

SA_EMAIL="${SA_NAME}@${PROJECT_ID}.iam.gserviceaccount.com"
if ! gcloud iam service-accounts describe "$SA_EMAIL" --project="$PROJECT_ID" --quiet >/dev/null 2>&1; then
  echo "==> Creating deployer service account: $SA_EMAIL"
  gcloud iam service-accounts create "$SA_NAME" \
    --project="$PROJECT_ID" \
    --display-name="GitHub Actions SVF Deployer" \
    --description="Keyless GitHub Actions deployer for ${GITHUB_REPO}" \
    --quiet
else
  echo "==> Service account already exists: $SA_EMAIL"
fi

echo "==> Granting least-privilege Compute Engine SSH/SCP roles to $SA_EMAIL ..."
for ROLE in \
  roles/compute.instanceAdmin.v1 \
  roles/compute.osAdminLogin \
  roles/iam.serviceAccountUser \
  roles/iap.tunnelResourceAccessor; do
  gcloud projects add-iam-policy-binding "$PROJECT_ID" \
    --member="serviceAccount:${SA_EMAIL}" \
    --role="$ROLE" \
    --condition=None \
    --quiet >/dev/null
done

if ! gcloud iam workload-identity-pools describe "$POOL_NAME" \
  --project="$PROJECT_ID" --location="global" --quiet >/dev/null 2>&1; then
  echo "==> Creating Workload Identity Pool: $POOL_NAME"
  gcloud iam workload-identity-pools create "$POOL_NAME" \
    --project="$PROJECT_ID" \
    --location="global" \
    --display-name="GitHub Actions Pool" \
    --description="Workload Identity Pool for ${GITHUB_REPO}" \
    --quiet
else
  echo "==> Workload Identity Pool already exists: $POOL_NAME"
fi

if ! gcloud iam workload-identity-pools providers describe "$PROVIDER_NAME" \
  --project="$PROJECT_ID" --location="global" --workload-identity-pool="$POOL_NAME" --quiet >/dev/null 2>&1; then
  echo "==> Creating GitHub OIDC Provider restricted to ${GITHUB_REPO} ..."
  gcloud iam workload-identity-pools providers create-oidc "$PROVIDER_NAME" \
    --project="$PROJECT_ID" \
    --location="global" \
    --workload-identity-pool="$POOL_NAME" \
    --display-name="GitHub Actions OIDC" \
    --description="OIDC provider restricted to ${GITHUB_REPO}" \
    --issuer-uri="https://token.actions.githubusercontent.com" \
    --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" \
    --attribute-condition="assertion.repository == '${GITHUB_REPO}'" \
    --quiet
else
  echo "==> OIDC Provider already exists: $PROVIDER_NAME"
fi

echo "==> Allowing ${GITHUB_REPO} to impersonate ${SA_EMAIL} ..."
gcloud iam service-accounts add-iam-policy-binding "$SA_EMAIL" \
  --project="$PROJECT_ID" \
  --role="roles/iam.workloadIdentityUser" \
  --condition=None \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL_NAME}/attribute.repository/${GITHUB_REPO}" \
  --quiet >/dev/null

WIF_PROVIDER="projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL_NAME}/providers/${PROVIDER_NAME}"

echo ""
echo "=============================================================================="
echo "  Google Cloud Workload Identity Federation Ready!"
echo "=============================================================================="
echo "  GCP_PROJECT_ID                 = ${PROJECT_ID}"
echo "  GCP_ZONE                       = ${ZONE:-<YOUR_VM_ZONE>}"
echo "  GCP_VM_NAME                    = ${VM_NAME}"
echo "  GCP_DEPLOY_DIR                 = ${DEPLOY_DIR}"
echo "  GCP_SERVICE_ACCOUNT            = ${SA_EMAIL}"
echo "  GCP_WORKLOAD_IDENTITY_PROVIDER = ${WIF_PROVIDER}"
echo "=============================================================================="

if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  echo ""
  echo "==> Detected authenticated GitHub CLI (gh). Setting repository variables on ${GITHUB_REPO} ..."
  gh variable set GCP_PROJECT_ID --body "$PROJECT_ID" --repo "$GITHUB_REPO"
  if [ -n "$ZONE" ]; then
    gh variable set GCP_ZONE --body "$ZONE" --repo "$GITHUB_REPO"
  fi
  gh variable set GCP_VM_NAME --body "$VM_NAME" --repo "$GITHUB_REPO"
  gh variable set GCP_DEPLOY_DIR --body "$DEPLOY_DIR" --repo "$GITHUB_REPO"
  gh variable set GCP_SERVICE_ACCOUNT --body "$SA_EMAIL" --repo "$GITHUB_REPO"
  gh variable set GCP_WORKLOAD_IDENTITY_PROVIDER --body "$WIF_PROVIDER" --repo "$GITHUB_REPO"
  echo "==> All GitHub Actions repository variables configured on ${GITHUB_REPO}!"
else
  echo ""
  echo "Add the values above in GitHub -> ${GITHUB_REPO} -> Settings -> Secrets and variables -> Actions -> Variables."
  echo "(Optional variables: GCP_USE_IAP=true if SSH uses IAP tunneling, GCP_POST_DEPLOY_CMD='<command>' to restart a service.)"
fi
