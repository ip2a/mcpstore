import { useEffect, useState } from "react";
import {
  ExternalLinkIcon,
  LogInIcon,
  LogOutIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
} from "lucide-react";
import { toast } from "sonner";

import { SectionHeading } from "@/components/shared/section-heading";
import { Badge } from "@/components/ui/badge";
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

export function ServiceAuthPanel({ instanceId }: { instanceId: string }) {
  const { t } = useI18n();
  const authQuery = useServiceAuthQuery(instanceId);
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

  if (authQuery.isLoading) {
    return (
      <section className="border-b pb-4">
        <SectionHeading
          title={t("authentication")}
          titleAs="h2"
          className="border-b-0 pb-3"
        />
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner /> {t("loadingAuthenticationStatus")}
        </p>
      </section>
    );
  }

  if (authQuery.error || !auth) {
    return (
      <section className="border-b pb-4">
        <SectionHeading
          title={t("authentication")}
          titleAs="h2"
          className="border-b-0 pb-3"
        />
        <p className="text-sm text-destructive">
          {authQuery.error instanceof Error
            ? authQuery.error.message
            : t("authenticationStatusUnavailable")}
        </p>
        <Button
          className="mt-3"
          size="sm"
          variant="outline"
          onClick={() => authQuery.refetch()}
        >
          <RefreshCwIcon data-icon="inline-start" />
          {t("retry")}
        </Button>
      </section>
    );
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

  return (
    <section className="border-b pb-4">
      <SectionHeading
        title={t("authentication")}
        titleAs="h2"
        className="border-b-0 pb-3"
      />
      <div className="flex flex-wrap items-center gap-2">
        <Badge
          variant={auth.status === "authenticated" ? "default" : "outline"}
        >
          <ShieldCheckIcon />
          {authStatusLabel(auth.status, t)}
        </Badge>
        {auth.flow ? (
          <span className="font-mono text-xs text-muted-foreground">
            {auth.flow}
          </span>
        ) : null}
      </div>
      {auth.scopes.length ? (
        <p className="mt-3 break-words text-sm text-muted-foreground">
          {t("oauthScopes")}:{" "}
          <span className="font-mono">{auth.scopes.join(" ")}</span>
        </p>
      ) : null}
      {auth.required_scope ? (
        <p className="mt-2 break-words text-sm text-muted-foreground">
          {t("oauthRequiredScope")}:{" "}
          <span className="font-mono">{auth.required_scope}</span>
        </p>
      ) : null}
      <div className="mt-4 flex flex-wrap gap-2">
        {canLogin ? (
          <Button
            size="sm"
            onClick={() =>
              run("login", () => startInstanceAuthorization(instanceId), true)
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
                    instanceId,
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
              run("refresh", () => refreshInstanceAuthorization(instanceId))
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
              run("logout", () => logoutInstanceAuthorization(instanceId))
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
    </section>
  );
}
