//! A compact dropdown for the room control bar: a pill trigger showing the
//! current choice, and a popup of options that reveals on hover / keyboard
//! focus. Same idea (and roughly the same look) as the per-viewer quality
//! menu on a member card — a custom menu rather than a native `<select>`,
//! whose option list can't be themed to match the dark bar.

use leptos::prelude::*;

use crate::components::icons::{icon_bars, icon_chevron_down, icon_volume};

/// One option in a [`MenuSelect`]. `value` is the stable identity matched
/// against the control's `selected` signal; `label` is shown; `hint` is the
/// option's tooltip.
#[derive(Clone, Copy)]
pub struct MenuOption {
    pub value: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

/// The leading icon for a [`MenuSelect`]'s trigger. A small enum rather
/// than a view prop so the component's view type stays shallow — `RoomPage`
/// is already close to the type-recursion limit.
#[derive(Clone, Copy)]
pub enum MenuIcon {
    /// Bars — used for the video mode menu.
    Levels,
    /// Speaker — used for the audio quality menu.
    Volume,
}

#[component]
pub fn MenuSelect<F>(
    /// Accessible name / trigger tooltip for the whole control.
    label: &'static str,
    /// Small leading icon shown in the trigger.
    icon: MenuIcon,
    /// The options, top to bottom in the popup.
    options: Vec<MenuOption>,
    /// The currently selected `value`.
    #[prop(into)]
    selected: Signal<&'static str>,
    /// Invoked with the chosen option's `value`. A plain `Fn` (not a
    /// `Callback`) so it can capture the non-`Send` `RoomSession`.
    on_select: F,
) -> impl IntoView
where
    F: Fn(&'static str) + Clone + 'static,
{
    let options_for_label = options.clone();
    let current_label = move || {
        let value = selected.get();
        options_for_label
            .iter()
            .find(|option| option.value == value)
            .map_or("", |option| option.label)
    };

    view! {
        <div class="menu-select">
            <button
                type="button"
                class="menu-select__trigger"
                title=label
                aria-label=label
                aria-haspopup="listbox"
            >
                {match icon {
                    MenuIcon::Levels => icon_bars().into_any(),
                    MenuIcon::Volume => icon_volume().into_any(),
                }}
                <span class="menu-select__current">{current_label}</span>
                {icon_chevron_down()}
            </button>
            <div class="menu-select__popup" role="listbox">
                {options
                    .into_iter()
                    .map(|option| {
                        let MenuOption { value, label, hint } = option;
                        let on_select = on_select.clone();
                        let is_on = move || selected.get() == value;
                        view! {
                            <button
                                type="button"
                                role="option"
                                class="menu-select__option"
                                class:menu-select__option--on=is_on
                                aria-selected=move || is_on().to_string()
                                title=hint
                                on:click=move |_| {
                                    on_select(value);
                                    crate::features::room::media_controls::blur_active_element();
                                }
                            >
                                {label}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}
