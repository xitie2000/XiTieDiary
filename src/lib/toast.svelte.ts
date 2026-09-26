export interface ToastItem {
  id: number;
  kind: 'error' | 'ok';
  text: string;
}

class ToastStore {
  items: ToastItem[] = $state([]);
  #next = 0;

  show(text: string, kind: 'error' | 'ok' = 'error') {
    const id = this.#next++;
    this.items.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), 4000);
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
