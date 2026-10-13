/** 判定结果：满足显示对勾，不满足显示破折号。 */
export function Flag({ ok }: { ok: boolean }) {
  return (
    <span className="flag" data-ok={ok}>
      {ok ? "✓" : "—"}
    </span>
  );
}
