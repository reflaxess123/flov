<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<div class="capsule">
  <span>{text}</span>
</div>

<style>
  .capsule {
    height: 40px;
    padding: 0 20px;
    border-radius: 999px;

    display: flex;
    align-items: center;
    justify-content: center;

    background: rgba(245, 245, 247, 0.92);
    color: #1c1c1e;
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.01em;

    backdrop-filter: blur(20px) saturate(180%);
    -webkit-backdrop-filter: blur(20px) saturate(180%);

    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 10px 30px rgba(0, 0, 0, 0.2),
      0 1px 3px rgba(0, 0, 0, 0.1);

    animation:
      pop-in 0.42s cubic-bezier(0.34, 1.56, 0.64, 1),
      breathe 2s ease-in-out 0.5s infinite;
  }

  @media (prefers-color-scheme: dark) {
    .capsule {
      background: rgba(28, 28, 30, 0.82);
      color: #f5f5f7;
      box-shadow:
        0 1px 0 rgba(255, 255, 255, 0.06) inset,
        0 10px 30px rgba(0, 0, 0, 0.55),
        0 1px 3px rgba(0, 0, 0, 0.4);
    }
  }

  @keyframes pop-in {
    0%   { transform: translateY(40px) scale(0.7); opacity: 0; }
    70%  { transform: translateY(-4px) scale(1.04); opacity: 1; }
    100% { transform: translateY(0) scale(1); opacity: 1; }
  }

  @keyframes breathe {
    0%, 100% { transform: scale(1); }
    50%      { transform: scale(1.015); }
  }
</style>
