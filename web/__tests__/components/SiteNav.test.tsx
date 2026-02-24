import { fireEvent, render, screen } from "@testing-library/react"
import { SiteNav } from "@/components/shell/SiteNav"

jest.mock("next/navigation", () => ({
  usePathname: jest.fn(() => "/"),
}))

describe("SiteNav mobile menu", () => {
  it("toggles mobile menu open and closed", () => {
    render(<SiteNav />)

    expect(screen.queryByLabelText(/mobile menu/i)).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole("button", { name: /toggle menu/i }))
    expect(screen.getByLabelText(/mobile menu/i)).toBeInTheDocument()

    fireEvent.click(screen.getByRole("button", { name: /toggle menu/i }))
    expect(screen.queryByLabelText(/mobile menu/i)).not.toBeInTheDocument()
  })
})

