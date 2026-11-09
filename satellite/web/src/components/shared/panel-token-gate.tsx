import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { browserLanguage, translate } from "@/lib/i18n-core";
import { setPanelToken, PANEL_AUTH_REQUIRED_EVENT } from "@/lib/api/client";

/**
 * Full-screen token prompt shown whenever the panel API answers 401
 * (non-loopback panel binds enforce bearer auth). Saving reloads the page so
 * every in-flight query retries with the stored token. Mounted above the
 * i18n/meta providers: the 401 fires before those resolve anything, so the
 * gate resolves strings from the browser language directly.
 */
export function PanelTokenGate() {
  const [open, setOpen] = useState(false);
  const [token, setToken] = useState("");
  const t = (key: Parameters<typeof translate>[1]) => translate(browserLanguage(), key);

  useEffect(() => {
    const show = () => setOpen(true);
    window.addEventListener(PANEL_AUTH_REQUIRED_EVENT, show);
    return () => window.removeEventListener(PANEL_AUTH_REQUIRED_EVENT, show);
  }, []);

  if (!open) return null;

  const save = () => {
    if (!token.trim()) return;
    setPanelToken(token);
    window.location.reload();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>{t("panelAuthTitle")}</CardTitle>
          <CardDescription>{t("panelAuthDescription")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Input
            autoFocus
            value={token}
            onChange={(event) => setToken(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") save();
            }}
            placeholder={t("panelAuthTokenPlaceholder")}
          />
          <Button onClick={save} disabled={!token.trim()}>
            {t("panelAuthSave")}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
