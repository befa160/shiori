import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Button } from './button'

describe('Button', () => {
  it('est un vrai bouton, cible tactile de 44 px par défaut', async () => {
    const onClick = vi.fn()
    render(<Button onClick={onClick}>Lire</Button>)
    const button = screen.getByRole('button', { name: 'Lire' })
    expect(button).toHaveClass('h-11')
    await userEvent.click(button)
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('le destructif garde un fond foncé lisible avec le texte blanc', () => {
    render(<Button variant="destructive">Supprimer</Button>)
    expect(screen.getByRole('button')).toHaveClass('bg-destructive-fill', 'text-white')
  })
})
