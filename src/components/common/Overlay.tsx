export function Overlay(props: { onClick: () => void; ariaLabel: string }) {
  return (
    <button type="button" class="overlay" aria-label={props.ariaLabel} onClick={props.onClick} />
  );
}
