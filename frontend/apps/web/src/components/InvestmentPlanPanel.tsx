import { useEffect, useState, type FormEvent } from "react";
import type { User } from "firebase/auth";
import {
  getQoinInvestmentPlan,
  saveQoinInvestmentPlan,
  type QoinInvestmentPlanInput,
} from "@qoin/core";

type Props = { user: User | null };

export function InvestmentPlanPanel({ user }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [goal, setGoal] = useState("");
  const [currency, setCurrency] = useState<QoinInvestmentPlanInput["currency"]>("KES");
  const [monthly, setMonthly] = useState("");
  const [target, setTarget] = useState("");
  const [years, setYears] = useState("10");
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (!user) {
      setSaved(false);
      setGoal("");
      setMonthly("");
      setTarget("");
      setYears("10");
      return () => { active = false; };
    }
    setSaved(false);
    setGoal("");
    setMonthly("");
    setTarget("");
    setYears("10");
    setMessage(null);
    setLoading(true);
    void user.getIdToken()
      .then(getQoinInvestmentPlan)
      .then((plan) => {
        if (!active || !plan) return;
        setGoal(plan.goal);
        setCurrency(plan.currency);
        setMonthly((plan.monthly_contribution_minor / 100).toString());
        setTarget(plan.target_amount_minor === null ? "" : (plan.target_amount_minor / 100).toString());
        setYears((plan.horizon_months / 12).toString());
        setSaved(true);
      })
      .catch(() => {
        if (active) setMessage("Your saved plan could not be loaded. Check the account service and try again.");
      })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [user]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage(null);
    if (!user) {
      setMessage("Sign in to save this plan securely to your Qoin account.");
      return;
    }
    const monthlyValue = Number(monthly);
    const targetValue = target.trim() ? Number(target) : null;
    const yearValue = Number(years);
    if (!goal.trim() || !Number.isFinite(monthlyValue) || monthlyValue <= 0 ||
        (targetValue !== null && (!Number.isFinite(targetValue) || targetValue <= 0)) ||
        !Number.isInteger(yearValue) || yearValue < 1 || yearValue > 50) {
      setMessage("Enter a goal, a positive monthly contribution, an optional positive target, and a horizon of 1–50 years.");
      return;
    }
    const plan: QoinInvestmentPlanInput = {
      goal: goal.trim(),
      currency,
      monthly_contribution_minor: Math.round(monthlyValue * 100),
      target_amount_minor: targetValue === null ? null : Math.round(targetValue * 100),
      horizon_months: yearValue * 12,
    };
    setBusy(true);
    try {
      await user.getIdToken().then((token) => saveQoinInvestmentPlan(plan, token));
      setSaved(true);
      setMessage("Your investment intention has been saved.");
      setExpanded(false);
    } catch {
      setMessage("The plan was not saved. Check your connection and try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="investment-plan-panel" aria-label="Investment intention">
      <div className="investment-plan-heading">
        <div>
          <p className="eyebrow">YOUR INVESTMENT INTENTION</p>
          <h3>{saved ? "Your plan" : "Start with your goal"}</h3>
          {saved ? <p>{goal} · {currency} {Number(monthly || 0).toLocaleString()} / month · {years} years</p> :
            <p>Set your own goal and contribution rhythm. No return is promised.</p>}
        </div>
        <button type="button" className="plan-toggle" onClick={() => setExpanded((value) => !value)} disabled={loading}>
          {expanded ? "Close" : saved ? "Edit" : "Set a goal"}
        </button>
      </div>
      {expanded ? (
        <form className="investment-plan-form" onSubmit={(event) => void submit(event)}>
          <label htmlFor="qoin-plan-goal">What are you investing towards?</label>
          <input id="qoin-plan-goal" maxLength={80} required value={goal} onChange={(event) => setGoal(event.target.value)} placeholder="e.g. Long-term financial security" />
          <div className="plan-field-grid">
            <div>
              <label htmlFor="qoin-plan-currency">Currency</label>
              <select id="qoin-plan-currency" value={currency} onChange={(event) => setCurrency(event.target.value as QoinInvestmentPlanInput["currency"])}>
                <option value="KES">KES</option><option value="USD">USD</option><option value="EUR">EUR</option><option value="GBP">GBP</option>
              </select>
            </div>
            <div>
              <label htmlFor="qoin-plan-monthly">Monthly contribution</label>
              <input id="qoin-plan-monthly" type="number" min="0.01" step="0.01" required value={monthly} onChange={(event) => setMonthly(event.target.value)} placeholder="3000" />
            </div>
            <div>
              <label htmlFor="qoin-plan-target">Target amount (optional)</label>
              <input id="qoin-plan-target" type="number" min="0.01" step="0.01" value={target} onChange={(event) => setTarget(event.target.value)} placeholder="e.g. 500000" />
            </div>
            <div>
              <label htmlFor="qoin-plan-years">Time horizon (years)</label>
              <input id="qoin-plan-years" type="number" min="1" max="50" step="1" required value={years} onChange={(event) => setYears(event.target.value)} />
            </div>
          </div>
          <button className="primary-action" type="submit" disabled={busy || loading}>{busy ? "Saving…" : "Save my plan"}</button>
          {!user ? <p className="plan-note">Sign in to securely save your plan across devices.</p> : null}
        </form>
      ) : null}
      {message ? <p className="plan-message" role="status">{message}</p> : null}
      <p className="plan-note">These are your own planning inputs. They do not trigger product recommendations, portfolio allocation or transactions.</p>
    </section>
  );
}
