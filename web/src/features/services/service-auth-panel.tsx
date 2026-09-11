import { useEffect, useState, type ReactNode } from "react";
import {
  ExternalLinkIcon,
  LogInIcon,
  LogOutIcon,
  RefreshCwIcon,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useServiceAuthQuery } from "@/features/services/queries";
import {
  logoutInstanceAuthorization,
  refreshInstanceAuthorization,
  startInstanceAuthorization,
  upgradeInstanceAuthorizationScope,
  type AuthOperationResult,
  type AuthStatus,
  type ServiceInstance,
} from "@/lib/api";
import { useI18n } from "@/lib/i18n-context";

type Translate = ReturnType<typeof useI18n>["t"];

function authStatusLabel(status: AuthStatus, t: Translate) {
  const labels: Record<AuthStatus, string> = {
    not_required: t("authStatusNotRequired"),
    unauthenticated: t("authStatusUnauthenticated"),
    authorizing: t("authStatusAuthorizing"),
    authenticated: t("authStatusAuthenticated"),
    refreshing: t("authStatusRefreshing"),
    scope_upgrade_required: t("authStatusScopeUpgradeRequired"),
    error: t("authStatusError"),
  };
  return labels[status];
}

function useServiceAuthController(service: ServiceInstance) {
  const { t } = useI18n();
  const authQuery = useServiceAuthQuery(service);
  const [busy, setBusy] = useState<string | null>(null);
  const auth = authQuery.data;

  useEffect(() => {
    if (auth?.status !== "authorizing" && auth?.status !== "refreshing") return;
    const timer = window.setInterval(() => void authQuery.refetch(), 1500);
    return () => window.clearInterval(timer);
  }, [auth?.status, authQuery.refetch]);

  async function run(
    label: string,
    action: () => Promise<AuthOperationResult>,
    expectsAuthorizationUrl = false,
  ) {
    const popup = expectsAuthorizationUrl
      ? window.open("about:blank", "_blank")
      : null;
    if (popup) popup.opener = null;

    setBusy(label);
    try {
      const result = await action();
      await authQuery.refetch();
      if (result.authorization?.authorization_url) {
        if (popup)
          popup.location.replace(result.authorization.authorization_url);
        else toast.info(t("oauthPopupBlocked"));
      } else {
        popup?.close();
      }
    } catch (error) {
      popup?.close();
      toast.error(
        error instanceof Error ? error.message : t("oauthActionFailed"),
      );
      await authQuery.refetch();
    } finally {
      setBusy(null);
    }
  }

  return { t, authQuery, auth, busy, run };
}

function ServiceAuthStatusView({
  t,
  authQuery,
  auth,
}: ReturnType<typeof useServiceAuthController>) {
  if (authQuery.isLoading) {
    return (
      <span className="inline-flex items-center gap-2 text-sm text-muted-foreground">
        <Spinner /> {t("loadingAuthenticationStatus")}
      </span>
    );
  }

  if (authQuery.error || !auth) {
    return (
      <span className="text-sm text-destructive">
        {authQuery.error instanceof Error
          ? authQuery.error.message
          : t("authenticationStatusUnavailable")}
      </span>
    );
  }

  return (
    <span className="inline-flex flex-wrap items-center gap-2 text-sm">
      <span>{authStatusLabel(auth.status, t)}</span>
      {auth.flow ? (
        <span className="font-mono text-xs text-muted-foreground">
          {auth.flow}
        </span>
      ) : null}
    </span>
  );
}

function ServiceAuthActionsView({
  service,
  controller,
}: {
  service: ServiceInstance;
  controller: ReturnType<typeof useServiceAuthController>;
}) {
  const { t, authQuery, auth, busy, run } = controller;

  if (authQuery.isLoading || authQuery.error || !auth) {
    if (authQuery.error || (!authQuery.isLoading && !auth)) {
      return (
        <Button
          size="sm"
          variant="outline"
          className="w-fit"
          onClick={() => authQuery.refetch()}
        >
          <RefreshCwIcon data-icon="inline-start" />
          {t("retry")}
        </Button>
      );
    }
    return null;
  }

  const pending =
    Boolean(busy) ||
    auth.status === "authorizing" ||
    auth.status === "refreshing";
  const canLogin = auth.status === "unauthenticated" || auth.status === "error";
  const canRefresh = auth.status === "authenticated";
  const canLogout =
    auth.status === "authenticated" || auth.status === "scope_upgrade_required";
  const canUpgrade =
    auth.status === "scope_upgrade_required" && Boolean(auth.required_scope);
  const hasMeta =
    Boolean(auth.scopes?.length) ||
    Boolean(auth.required_scope) ||
    canLogin ||
    canUpgrade ||
    canRefresh ||
    canLogout ||
    auth.status === "authorizing";

  if (!hasMeta) return null;

  return (
    <div className="flex flex-col gap-3">
      {auth.scopes?.length ? (
        <p className="break-words text-sm text-muted-foreground">
          {t("oauthScopes")}:{" "}
          <span className="font-mono">{auth.scopes.join(" ")}</span>
        </p>
      ) : null}
      {auth.required_scope ? (
        <p className="break-words text-sm text-muted-foreground">
          {t("oauthRequiredScope")}:{" "}
          <span className="font-mono">{auth.required_scope}</span>
        </p>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {canLogin ? (
          <Button
            size="sm"
            onClick={() =>
              run("login", () => startInstanceAuthorization(service), true)
            }
            disabled={pending}
          >
            {busy === "login" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <LogInIcon data-icon="inline-start" />
            )}
            {t("oauthLogin")}
          </Button>
        ) : null}
        {canUpgrade ? (
          <Button
            size="sm"
            onClick={() =>
              run(
                "scope",
                () =>
                  upgradeInstanceAuthorizationScope(
                    service,
                    auth.required_scope!,
                  ),
                true,
              )
            }
            disabled={pending}
          >
            {busy === "scope" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <ExternalLinkIcon data-icon="inline-start" />
            )}
            {t("oauthUpgradeScope")}
          </Button>
        ) : null}
        {canRefresh ? (
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              run("refresh", () => refreshInstanceAuthorization(service))
            }
            disabled={pending}
          >
            {busy === "refresh" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCwIcon data-icon="inline-start" />
            )}
            {t("refreshAuthorization")}
          </Button>
        ) : null}
        {canLogout ? (
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              run("logout", () => logoutInstanceAuthorization(service))
            }
            disabled={pending}
          >
            {busy === "logout" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <LogOutIcon data-icon="inline-start" />
            )}
            {t("oauthLogout")}
          </Button>
        ) : null}
        {auth.status === "authorizing" ? (
          <Button
            size="sm"
            variant="outline"
            onClick={() => authQuery.refetch()}
            disabled={Boolean(busy)}
          >
            <RefreshCwIcon data-icon="inline-start" />
            {t("refreshAuthorizationStatus")}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

export function ServiceAuthConnectionFields({
  service,
  render,
}: {
  service: ServiceInstance;
  render: (parts: { statusField: ReactNode; actions: ReactNode }) => ReactNode;
}) {
  const controller = useServiceAuthController(service);
  const { t } = controller;

  return render({
    statusField: (
      <div className="grid gap-1">
        <dt className="font-mono text-xs uppercase text-muted-foreground">
          {t("authentication")}
        </dt>
        <dd className="text-sm">
          <ServiceAuthStatusView {...controller} />
        </dd>
      </div>
    ),
    actions: (
      <ServiceAuthActionsView service={service} controller={controller} />
    ),
  });
}

export function ServiceAuthPanel({ service }: { service: ServiceInstance }) {
  const controller = useServiceAuthController(service);

  return (
    <div className="flex flex-col gap-3">
      <ServiceAuthStatusView {...controller} />
      <ServiceAuthActionsView service={service} controller={controller} />
    </div>
  );
}
