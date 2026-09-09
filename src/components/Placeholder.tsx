/**
 * 아직 만들지 않은 화면. 어느 단계에서 채워지는지 적어 둔다.
 * (개발 중에만 보이는 화면이므로 솔직하게 적는 편이 낫다.)
 */
export default function Placeholder({ title, phase, note }: { title: string; phase: string; note?: string }) {
  return (
    <div className="page">
      <h1 className="page-title">{title}</h1>
      <div className="placeholder">
        <div className="placeholder-badge">{phase} 에서 만듭니다</div>
        {note && <p className="placeholder-note">{note}</p>}
      </div>
    </div>
  )
}
