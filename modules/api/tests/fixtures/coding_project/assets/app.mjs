document.body.dataset.ready = 'true';
document.querySelector('#toggle')?.addEventListener('click', () => {
  document.body.dataset.toggled = 'true';
  document.querySelector('#toggle').textContent = '状态已切换';
});
